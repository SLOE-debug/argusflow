//! 每适配器一个硬件设备，无软件或其他采集后端回退。
use super::{Difference, failure};
use argusflow_capture_contracts::{ByteBudget, CaptureResult};
use windows::{
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::*,
            Dxgi::IDXGIAdapter1,
        },
    },
    core::Interface,
};
pub(in crate::capture) struct Graphics {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub budget: ByteBudget,
    pub difference: Difference,
}
impl Graphics {
    pub fn new(adapter: &IDXGIAdapter1, budget: ByteBudget) -> CaptureResult<Self> {
        let mut device = None;
        let mut context = None;
        // SAFETY: 设备绑定枚举出的硬件适配器，输出槽仅在本线程访问。
        unsafe {
            D3D11CreateDevice(
                adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        }
        .map_err(failure)?;
        let device = device.ok_or_else(|| super::invalid("missing D3D11 device"))?;
        let context = context.ok_or_else(|| super::invalid("missing D3D11 context"))?;
        // 验证绑定适配器，不允许隐式选择另一块显卡。
        let _: windows::Win32::Graphics::Dxgi::IDXGIDevice = device.cast().map_err(failure)?;
        let difference = Difference::new(&device)?;
        Ok(Self {
            device,
            context,
            budget,
            difference,
        })
    }
}
