//! 互斥串行访问的多显示器桌面采集编排，不包含 GDI 回退或后台轮询。

use super::{
    desktop_device::DesktopDevice,
    desktop_output::DesktopOutput,
    desktop_pixels::{OutputCrop, Rotation},
};
use argusflow_core::{EvidenceFrame, EvidencePixelFormat, InspectionFailure, InspectionRect};
use windows::{
    Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ERROR_NOT_FOUND, DXGI_OUTPUT_DESC, IDXGIFactory1, IDXGIOutput1,
    },
    core::Interface,
};

/// 由 WindowsEventCapture 的 Mutex 独占访问；不允许并发调用 immediate context。
pub(super) struct EvidenceDesktop {
    /// 用于检测适配器拓扑变化，变化时上层会撤销整个状态。
    factory: IDXGIFactory1,
    /// 设备按适配器共享，不跨显卡复制资源。
    adapters: Vec<DesktopAdapter>,
}

/// 同一显卡上的多个输出共享设备，会话仍按输出隔离。
struct DesktopAdapter {
    graphics: DesktopDevice,
    outputs: Vec<OutputState>,
}

/// 显示器句柄可枚举后保留；只在截图覆盖它时创建复制会话。
struct OutputState {
    output: IDXGIOutput1,
    /// 初始几何用于检测分辨率、位置和旋转变化。
    description: OutputDescription,
    capture: Option<DesktopOutput>,
    /// 流式来源独立持有复制租约，同步截图不能消费掉其变化。
    stream_capture: Option<DesktopOutput>,
    updates: super::desktop_updates::DesktopUpdateQueue,
}

impl EvidenceDesktop {
    pub(super) fn sources(&mut self) -> Vec<argusflow_core::CaptureSourceId> {
        for adapter in &mut self.adapters {
            for output in &mut adapter.outputs {
                output.updates.request_baseline();
            }
        }
        self.adapters
            .iter()
            .flat_map(|adapter| adapter.outputs.iter())
            .map(|output| argusflow_core::CaptureSourceId(output.description.monitor as u64))
            .collect()
    }
    /// 非阻塞推进全部显示器，保留每个输出独立的呈现时间。
    pub(super) fn poll_updates(
        &mut self,
        generation: argusflow_core::CaptureGeneration,
    ) -> Result<Vec<argusflow_core::capture::ScreenCaptureUpdate>, InspectionFailure> {
        self.advance_updates(generation, true)
    }
    pub(super) fn drain_updates(
        &mut self,
        generation: argusflow_core::CaptureGeneration,
    ) -> Result<(Vec<argusflow_core::capture::ScreenCaptureUpdate>, bool), InspectionFailure> {
        let updates = self.advance_updates(generation, false)?;
        let pending = self
            .adapters
            .iter()
            .flat_map(|adapter| &adapter.outputs)
            .any(|output| output.updates.pending());
        Ok((updates, pending))
    }
    fn advance_updates(
        &mut self,
        generation: argusflow_core::CaptureGeneration,
        acquire: bool,
    ) -> Result<Vec<argusflow_core::capture::ScreenCaptureUpdate>, InspectionFailure> {
        if !unsafe { self.factory.IsCurrent() }.as_bool() {
            return Err(InspectionFailure::ContextChanged);
        }
        let mut updates = Vec::new();
        for adapter in &mut self.adapters {
            for output in &mut adapter.outputs {
                output.validate()?;
                if output.stream_capture.is_none() {
                    output.stream_capture =
                        Some(DesktopOutput::new(&output.output, &adapter.graphics)?);
                }
                let capture = output
                    .stream_capture
                    .as_mut()
                    .ok_or(InspectionFailure::Unavailable)?;
                if let Some(update) = output.updates.poll(
                    capture,
                    &adapter.graphics,
                    output.description.bounds,
                    Rotation::try_from(output.description.rotation)?,
                    argusflow_core::CaptureSourceId(output.description.monitor as u64),
                    generation,
                    acquire,
                )? {
                    updates.push(update);
                }
            }
        }
        Ok(updates)
    }

    /// 建立当前桌面的显示器清单，不生成后台线程，也不预读屏幕内容。
    pub(super) fn new() -> Result<Self, InspectionFailure> {
        // SAFETY: 工厂返回引用计数接口，由本对象独占使用。
        let factory: IDXGIFactory1 =
            unsafe { CreateDXGIFactory1() }.map_err(|_| InspectionFailure::Unavailable)?;
        let mut adapters = Vec::new();
        for index in 0.. {
            let adapter = match unsafe { factory.EnumAdapters1(index) } {
                Ok(adapter) => adapter,
                Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(_) => return Err(InspectionFailure::Unavailable),
            };
            let mut outputs = Vec::new();
            for index in 0.. {
                let output = match unsafe { adapter.EnumOutputs(index) } {
                    Ok(output) => output,
                    Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                    Err(_) => return Err(InspectionFailure::Unavailable),
                };
                let description =
                    unsafe { output.GetDesc() }.map_err(|_| InspectionFailure::Unavailable)?;
                if !description.AttachedToDesktop.as_bool() {
                    continue;
                }
                outputs.push(OutputState {
                    output: output.cast().map_err(|_| InspectionFailure::Unavailable)?,
                    description: OutputDescription::from(description),
                    capture: None,
                    stream_capture: None,
                    updates: Default::default(),
                });
            }
            if !outputs.is_empty() {
                adapters.push(DesktopAdapter {
                    graphics: DesktopDevice::new(&adapter)?,
                    outputs,
                });
            }
        }
        if adapters.is_empty() {
            return Err(InspectionFailure::Unavailable);
        }
        Ok(Self { factory, adapters })
    }

    /// 先向所有命中输出提交 GPU 复制，再逐个读回，跨屏截图保留未覆盖区域的黑色。
    pub(super) fn capture(
        &mut self,
        bounds: InspectionRect,
    ) -> Result<EvidenceFrame, InspectionFailure> {
        self.capture_update(bounds, false)?
            .ok_or(InspectionFailure::Unavailable)
    }

    /// 无新呈现时不读回、不分配和复制整帧。
    pub(super) fn capture_update(
        &mut self,
        bounds: InspectionRect,
        only_changed: bool,
    ) -> Result<Option<EvidenceFrame>, InspectionFailure> {
        if !unsafe { self.factory.IsCurrent() }.as_bool() {
            return Err(InspectionFailure::ContextChanged);
        }
        // 上层提供裁到虚拟桌面后的整数物理范围；此处仍限制分配边界。
        if !bounds.is_valid() || bounds.width * bounds.height > 32_000_000.0 {
            return Err(InspectionFailure::InvalidGeometry);
        }
        let width = bounds.width as u32;
        let height = bounds.height as u32;
        let mut crops = Vec::new();
        let mut changed = false;
        for (adapter_index, adapter) in self.adapters.iter_mut().enumerate() {
            for (output_index, output) in adapter.outputs.iter_mut().enumerate() {
                output.validate()?;
                let rotation = Rotation::try_from(output.description.rotation)?;
                let Some(crop) = OutputCrop::new(bounds, output.description.bounds, rotation)
                else {
                    continue;
                };
                if output.capture.is_none() {
                    output.capture = Some(DesktopOutput::new(&output.output, &adapter.graphics)?);
                }
                changed |= output
                    .capture
                    .as_mut()
                    .ok_or(InspectionFailure::Unavailable)?
                    .refresh(&adapter.graphics)?;
                crops.push((adapter_index, output_index, crop));
            }
        }
        if crops.is_empty() {
            return Err(InspectionFailure::InvalidGeometry);
        }
        if only_changed && !changed {
            return Ok(None);
        }
        let mut pixels = vec![0; width as usize * height as usize * 4];
        for (adapter_index, output_index, crop) in crops {
            let adapter = &mut self.adapters[adapter_index];
            adapter.outputs[output_index]
                .capture
                .as_mut()
                .ok_or(InspectionFailure::Unavailable)?
                .read(
                    &adapter.graphics,
                    &crop.texture_region(),
                    |source, stride, width, height| {
                        crop.copy_cropped(source, stride, width, height, &mut pixels)
                    },
                )?;
        }
        EvidenceFrame::new(bounds, width, height, EvidencePixelFormat::Bgrx8, pixels).map(Some)
    }
}

impl OutputState {
    /// 不让旧 staging 在显示器旋转、移动或断开后继续冒充当前桌面。
    fn validate(&self) -> Result<(), InspectionFailure> {
        // SAFETY: 仅查询当前输出的桌面状态。
        let current =
            unsafe { self.output.GetDesc() }.map_err(|_| InspectionFailure::Unavailable)?;
        if !current.AttachedToDesktop.as_bool()
            || OutputDescription::from(current) != self.description
        {
            return Err(InspectionFailure::ContextChanged);
        }
        Ok(())
    }
}

/// 显示器查询快照只保存值；HMONITOR 仅用于身份比较，不持有或跨线程解引用裸指针。
#[derive(PartialEq)]
struct OutputDescription {
    /// 物理桌面位置与尺寸。
    bounds: windows::Win32::Foundation::RECT,
    /// 驱动报告的旋转值，裁切前再转换为封闭方向类型。
    rotation: windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_ROTATION,
    /// 无所有权的显示器身份标记。
    monitor: usize,
}
impl From<DXGI_OUTPUT_DESC> for OutputDescription {
    fn from(value: DXGI_OUTPUT_DESC) -> Self {
        Self {
            bounds: value.DesktopCoordinates,
            rotation: value.Rotation,
            monitor: value.Monitor.0 as usize,
        }
    }
}
