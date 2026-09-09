//! 单个显示输出的帧租约与区域读回；仅 DXGI 确认没有桌面更新时复用已采样内容。

use super::{desktop_device::DesktopDevice, desktop_readback::DesktopReadback};
use argusflow_core::InspectionFailure;
use windows::{
    Win32::Graphics::{
        Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Texture2D},
        Dxgi::{
            Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
            DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO, IDXGIOutput1, IDXGIOutputDuplication,
        },
    },
    core::Interface,
};

/// 首帧必须取得真实桌面呈现；GPU 桌面和 CPU 区域缓冲均独立于复制租约。
pub(super) struct DesktopOutput {
    /// 桌面切换或设备失效时由上层丢弃整个采集状态。
    duplication: IDXGIOutputDuplication,
    /// 区域分配和映射由独立读回模块管理。
    readback: DesktopReadback,
    /// GPU 上保存完整桌面；只将命中窗口的区域复制到 staging。
    desktop: Option<ID3D11Texture2D>,
    /// 原始未旋转纹理尺寸，用于核对桌面拓扑和行布局。
    width: u32,
    height: u32,
    /// 当前呈现相对上一个租约的系统变化候选。
    damage: Vec<windows::Win32::Foundation::RECT>,
    /// 原生帧统计供录制判断是否已经发生不可恢复的合并。
    accumulated_frames: u32,
    /// 原生 QPC 呈现时钟，不以 CPU 读回结束冒充呈现。
    presented_qpc: i64,
}

impl DesktopOutput {
    /// 只在实际命中的显示器上开启复制会话。
    pub(super) fn new(
        output: &IDXGIOutput1,
        graphics: &DesktopDevice,
    ) -> Result<Self, InspectionFailure> {
        // SAFETY: device 与 output 属于同一硬件适配器。
        let duplication = unsafe { output.DuplicateOutput(&graphics.device) }
            .map_err(|_| InspectionFailure::Unavailable)?;
        Ok(Self {
            duplication,
            readback: DesktopReadback::default(),
            desktop: None,
            width: 0,
            height: 0,
            damage: Vec::new(),
            accumulated_frames: 0,
            presented_qpc: 0,
        })
    }

    /// 获取当前可用桌面；静态桌面不等待下一次刷新，失败绝不伪装为旧帧成功。
    pub(super) fn refresh(&mut self, graphics: &DesktopDevice) -> Result<bool, InspectionFailure> {
        self.refresh_inner(graphics, true)
    }

    pub(super) fn refresh_now(
        &mut self,
        graphics: &DesktopDevice,
    ) -> Result<bool, InspectionFailure> {
        self.refresh_inner(graphics, false)
    }

    fn refresh_inner(
        &mut self,
        graphics: &DesktopDevice,
        wait_initial: bool,
    ) -> Result<bool, InspectionFailure> {
        self.damage.clear();
        self.accumulated_frames = 0;
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
        // 首次订阅允许有限等待；已初始化时只读取积累的最新更新，不人为等待 VSync。
        loop {
            let timeout_ms = if self.desktop.is_some() || !wait_initial {
                0
            } else {
                deadline
                    .saturating_duration_since(std::time::Instant::now())
                    .as_millis() as u32
            };
            // SAFETY: 输出槽在本线程有效，前一个租约已由 FrameLease 释放。
            match unsafe {
                self.duplication
                    .AcquireNextFrame(timeout_ms, &mut info, &mut resource)
            } {
                Err(error) if error.code() == DXGI_ERROR_WAIT_TIMEOUT && self.desktop.is_some() => {
                    return Ok(false);
                }
                Err(error) if error.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    return Err(InspectionFailure::Timeout);
                }
                Err(_) => return Err(InspectionFailure::Unavailable),
                Ok(()) => {}
            }
            // 鼠标更新也能产生成功租约，但 LastPresentTime=0 的资源不包含新桌面像素。
            // 首帧必须等到真实呈现；已初始化时保留上次确认的桌面，不复制未定义内容。
            if info.LastPresentTime == 0 {
                drop(FrameLease(&self.duplication));
                resource = None;
                if self.desktop.is_some() {
                    return Ok(false);
                }
                if !wait_initial {
                    return Ok(false);
                }
                if std::time::Instant::now() >= deadline {
                    return Err(InspectionFailure::Timeout);
                }
                continue;
            }
            break;
        }
        let lease = FrameLease(&self.duplication);
        self.damage =
            super::desktop_damage::regions(&self.duplication, info.TotalMetadataBufferSize)?;
        self.accumulated_frames = info.AccumulatedFrames;
        self.presented_qpc = info.LastPresentTime;
        let source: ID3D11Texture2D = resource
            .ok_or(InspectionFailure::Unavailable)?
            .cast()
            .map_err(|_| InspectionFailure::Unavailable)?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: source 是当前尚未释放的复制帧。
        unsafe { source.GetDesc(&mut desc) };
        if desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
            || desc.SampleDesc.Count != 1
            || desc.Width == 0
            || desc.Height == 0
            || u64::from(desc.Width) * u64::from(desc.Height) > 32_000_000
        {
            return Err(InspectionFailure::InvalidGeometry);
        }
        if self.desktop.is_some() && (self.width != desc.Width || self.height != desc.Height) {
            return Err(InspectionFailure::ContextChanged);
        }
        let baseline = self.desktop.is_none();
        if baseline {
            let desktop_desc = D3D11_TEXTURE2D_DESC {
                Width: desc.Width,
                Height: desc.Height,
                MipLevels: 1,
                ArraySize: 1,
                Format: desc.Format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE_DEFAULT,
                ..Default::default()
            };
            // SAFETY: 全部资源参数由已校验的复制纹理导出，输出槽由本会话独占。
            unsafe {
                graphics
                    .device
                    .CreateTexture2D(&desktop_desc, None, Some(&mut self.desktop))
            }
            .map_err(|_| InspectionFailure::Unavailable)?;
            self.width = desc.Width;
            self.height = desc.Height;
        }
        let desktop = self
            .desktop
            .as_ref()
            .ok_or(InspectionFailure::Unavailable)?;
        // SAFETY: 相同设备、尺寸和格式，在租约结束前提交独立 GPU 桌面副本。
        if baseline || self.damage.is_empty() {
            unsafe { graphics.context.CopyResource(desktop, &source) };
        } else {
            for rect in &self.damage {
                if rect.left < 0
                    || rect.top < 0
                    || rect.right > self.width as i32
                    || rect.bottom > self.height as i32
                    || rect.left >= rect.right
                    || rect.top >= rect.bottom
                {
                    return Err(InspectionFailure::InvalidGeometry);
                }
                let region = windows::Win32::Graphics::Direct3D11::D3D11_BOX {
                    left: rect.left as u32,
                    top: rect.top as u32,
                    front: 0,
                    right: rect.right as u32,
                    bottom: rect.bottom as u32,
                    back: 1,
                };
                // 移动目的地和脏区均取当前最终像素，不在 CPU 重放移动指令。
                unsafe {
                    graphics.context.CopySubresourceRegion(
                        desktop,
                        0,
                        region.left,
                        region.top,
                        0,
                        &source,
                        0,
                        Some(&region),
                    );
                }
            }
        }
        unsafe { graphics.context.Flush() };
        self.readback.invalidate();
        drop(lease);
        Ok(true)
    }

    /// 返回系统确认的变化候选，不把粗矩形直接当作精确像素差分。
    pub(super) fn damage(&self) -> &[windows::Win32::Foundation::RECT] {
        &self.damage
    }

    /// 当前租约覆盖的呈现次数；大于一表示中间画面已经合并。
    pub(super) fn accumulated_frames(&self) -> u32 {
        self.accumulated_frames
    }

    /// 最近真实桌面呈现的 QPC 时间。
    pub(super) fn presented_qpc(&self) -> i64 {
        self.presented_qpc
    }

    pub(super) fn texture(&self) -> Result<&ID3D11Texture2D, InspectionFailure> {
        self.desktop.as_ref().ok_or(InspectionFailure::Unavailable)
    }

    /// 映射后通过借用闭包消费；无论复制成功与否都解除映射。
    pub(super) fn read<T>(
        &mut self,
        graphics: &DesktopDevice,
        region: &windows::Win32::Graphics::Direct3D11::D3D11_BOX,
        consume: impl FnOnce(&[u8], usize, u32, u32) -> Result<T, InspectionFailure>,
    ) -> Result<T, InspectionFailure> {
        if region.right > self.width
            || region.bottom > self.height
            || region.left >= region.right
            || region.top >= region.bottom
        {
            return Err(InspectionFailure::InvalidGeometry);
        }
        let desktop = self
            .desktop
            .as_ref()
            .ok_or(InspectionFailure::Unavailable)?;
        self.readback.read(graphics, desktop, region, consume)
    }
}

/// 成功 Acquire 后的线性租约；所有错误路径也必须恰好 Release 一次。
struct FrameLease<'a>(&'a IDXGIOutputDuplication);
impl Drop for FrameLease<'_> {
    fn drop(&mut self) {
        // SAFETY: 此守卫只在 AcquireNextFrame 成功后构造，不泄漏帧资源给调用方。
        unsafe {
            let _ = self.0.ReleaseFrame();
        }
    }
}
