//! D3D11 设备初始化；只向上层暴露可用于 WGC readback 的接口。

use windows::{
    Graphics::DirectX::Direct3D11::IDirect3DDevice,
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL},
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device, ID3D11DeviceContext,
            },
            Dxgi::{IDXGIAdapter, IDXGIDevice},
        },
        System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice,
    },
    core::Interface,
};

use argusflow_vision::VisionError;

use super::error::capture_error;

/// WGC 共享的 D3D11 设备与 immediate context。
#[derive(Debug)]
pub(super) struct GraphicsDevice {
    /// D3D11 设备，用于创建 staging texture。
    pub device: ID3D11Device,
    /// immediate context，用于 CopyResource/Map。
    pub context: ID3D11DeviceContext,
    /// WinRT WGC 需要的 IDirect3DDevice。
    pub capture_device: IDirect3DDevice,
}

/// 在当前图形适配器上创建支持 BGRA 的硬件 D3D11 设备。
pub(super) fn create_graphics_device() -> Result<GraphicsDevice, VisionError> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    // SAFETY: 输出指针指向当前栈帧中的 Option 接收槽；不传入应用内存地址。
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Option::<&[D3D_FEATURE_LEVEL]>::None,
            D3D11_SDK_VERSION,
            Some(&mut device as *mut Option<ID3D11Device>),
            None,
            Some(&mut context as *mut Option<ID3D11DeviceContext>),
        )
    }
    .map_err(|error| capture_error("failed to create D3D11 device", error))?;
    let device = device.ok_or_else(|| super::error::invalid_capture("D3D11 returned no device"))?;
    let context = context
        .ok_or_else(|| super::error::invalid_capture("D3D11 returned no immediate context"))?;
    let dxgi_device: IDXGIDevice = device
        .cast()
        .map_err(|error| capture_error("failed to query IDXGIDevice", error))?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }
        .map_err(|error| capture_error("failed to create WinRT Direct3D device", error))?;
    let capture_device = inspectable
        .cast::<IDirect3DDevice>()
        .map_err(|error| capture_error("failed to cast WinRT Direct3D device", error))?;
    Ok(GraphicsDevice {
        device,
        context,
        capture_device,
    })
}
