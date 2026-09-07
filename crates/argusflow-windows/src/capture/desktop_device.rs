//! Desktop Duplication 的适配器设备创建，访问由截图实例的 Mutex 串行化。

use argusflow_core::InspectionFailure;
use windows::Win32::{
    Foundation::HMODULE,
    Graphics::{
        Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
        Direct3D11::{
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
            ID3D11DeviceContext,
        },
        Dxgi::IDXGIAdapter1,
    },
};

/// 每个适配器创建一次；immediate context 只允许互斥的同步访问。
pub(super) struct DesktopDevice {
    /// 资源与输出必须属于同一适配器，不能总是选择默认显卡。
    pub(super) device: ID3D11Device,
    /// 拷贝和映射不可并发，调用方必须持有截图实例的互斥锁。
    pub(super) context: ID3D11DeviceContext,
}

impl DesktopDevice {
    /// 在输出所属的硬件适配器上建设备；失败直接报告，不降级到 GDI。
    pub(super) fn new(adapter: &IDXGIAdapter1) -> Result<Self, InspectionFailure> {
        let mut device = None;
        let mut context = None;
        // SAFETY: adapter 有效，两个输出槽由当前栈独占；显式适配器要求 UNKNOWN 驱动类型。
        unsafe {
            D3D11CreateDevice(
                adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        }
        .map_err(|_| InspectionFailure::Unavailable)?;
        Ok(Self {
            device: device.ok_or(InspectionFailure::Unavailable)?,
            context: context.ok_or(InspectionFailure::Unavailable)?,
        })
    }
}
