use super::*;
use argusflow_capture_contracts::*;
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

#[path = "scale.rs"]
mod scale;

fn graphics() -> Graphics {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.unwrap();
    let adapter = unsafe { factory.EnumAdapters1(0) }.unwrap();
    let desc = unsafe { adapter.GetDesc1() }.unwrap();
    println!(
        "D3D11 adapter: {}",
        String::from_utf16_lossy(
            &desc.Description[..desc
                .Description
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(desc.Description.len())]
        )
    );
    Graphics::new(&adapter, ByteBudget::new(256 * 1024 * 1024).unwrap()).unwrap()
}
fn upload(graphics: &Graphics, texture: &Texture, bytes: &[u8]) {
    let mut descriptor = windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.native.GetDesc(&mut descriptor) };
    unsafe {
        graphics.context.UpdateSubresource(
            &texture.native,
            0,
            None,
            bytes.as_ptr().cast(),
            descriptor.Width * 4,
            0,
        )
    };
}
