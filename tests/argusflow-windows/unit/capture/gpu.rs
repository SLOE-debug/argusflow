use super::*;
use argusflow_capture_contracts::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use windows::Win32::Graphics::{
    Direct3D11::D3D11_BOX,
    Dxgi::{CreateDXGIFactory1, IDXGIFactory1},
};

#[path = "benchmark.rs"]
mod benchmark;

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
    unsafe {
        graphics.context.UpdateSubresource(
            &texture.native,
            0,
            None,
            bytes.as_ptr().cast(),
            texture.width * 4,
            0,
        )
    };
}
fn diff(
    graphics: &Graphics,
    before: &Texture,
    after: &Texture,
    locations: &[[u32; 4]],
) -> PixelChanges {
    let pending = graphics
        .difference
        .submit(graphics, before, after, locations)
        .unwrap();
    unsafe { graphics.context.Flush() };
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some((result, _)) = pending.poll(graphics).unwrap() {
            return result;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
#[ignore = "requires hardware D3D11; explicit native GPU validation"]
fn exact_gpu_difference_and_snapshot_history() {
    let graphics = graphics();
    let before = Arc::new(Texture::new(&graphics, 67, 49, false).unwrap());
    let after = Arc::new(Texture::new(&graphics, 67, 49, false).unwrap());
    let old = vec![0; 67 * 49 * 4];
    let mut current = old.clone();
    for (x, y, value) in [(0, 0, 1), (66, 48, 2), (32, 16, 3), (9, 33, 4)] {
        current[(y * 67 + x) * 4] = value;
    }
    // 无意义 X 通道变化必须不触发 RGB 变化。
    for pixel in current.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    upload(&graphics, &before, &old);
    upload(&graphics, &after, &current);
    let regions = [
        PixelRect::new(0, 0, 67, 49).unwrap(),
        PixelRect::new(0, 0, 1, 1).unwrap(),
    ];
    let indices = tiles_for_regions(67, 49, &regions).unwrap();
    assert_eq!(indices.len(), 6);
    let locations: Vec<_> = indices
        .iter()
        .map(|i| {
            let x = *i as u32 % 3 * 32;
            let y = *i as u32 / 3 * 32;
            [x, y, x, y]
        })
        .collect();
    let result = diff(&graphics, &before, &after, &locations);
    assert_eq!(result.changed_pixels, 4);
    assert_eq!(result.compared_pixels, 67 * 49);
    for (x, y) in [(0, 0), (66, 48), (32, 16), (9, 33)] {
        assert!(
            result
                .regions
                .iter()
                .any(|r| r.x() <= x && r.right() > x && r.y() <= y && r.bottom() > y)
        );
    }
    assert_eq!(
        diff(&graphics, &after, &after, &locations).changed_pixels,
        0
    );
    // 一像素宽笔画跨多个块，与 CPU 逐 RGB 比较计数一致。
    let mut strokes = current.clone();
    for y in 0..49 {
        strokes[(y * 67 + 31) * 4 + 1] = (y + 1) as u8;
    }
    let stroke_texture = Texture::new(&graphics, 67, 49, false).unwrap();
    upload(&graphics, &stroke_texture, &strokes);
    let expected = old
        .as_chunks::<4>()
        .0
        .iter()
        .zip(strokes.as_chunks::<4>().0)
        .filter(|(a, b)| a[..3] != b[..3])
        .count() as u64;
    assert_eq!(
        diff(&graphics, &before, &stroke_texture, &locations).changed_pixels,
        expected
    );
    let baseline = TileMap::initial(before.clone());
    let mut next = baseline.clone();
    let batch = TileBatch::freeze(&graphics, &after.native, 67, 49, &indices).unwrap();
    let pending = graphics
        .difference
        .submit(&graphics, &before, &batch.atlas, &batch.locations)
        .unwrap();
    unsafe { graphics.context.Flush() };
    let changed = loop {
        if let Some((_, changed)) = pending.poll(&graphics).unwrap() {
            break changed;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    batch.apply_compact(&graphics, &mut next, &changed).unwrap();
    let area = PixelRect::new(0, 0, 67, 49).unwrap();
    let cropped = next.crop(&graphics, area).unwrap();
    assert_eq!(
        diff(&graphics, &cropped, &after, &locations).changed_pixels,
        0
    );
    // 后续覆盖实时图像不应改变已经冻结的历史块。
    upload(&graphics, &after, &vec![19; 67 * 49 * 4]);
    let frozen = next.crop(&graphics, area).unwrap();
    assert_eq!(
        diff(&graphics, &cropped, &frozen, &locations).changed_pixels,
        0
    );
    let baseline = baseline.crop(&graphics, area).unwrap();
    assert_eq!(
        diff(&graphics, &baseline, &before, &locations).changed_pixels,
        0
    );
    // 确认 GPU 的局部复制与 CPU 读回尊重非紧密的 RowPitch。
    let source = Arc::new(Texture::new(&graphics, 7, 3, false).unwrap());
    let bytes: Vec<_> = (0..84).map(|i| i as u8).collect();
    upload(&graphics, &source, &bytes);
    let pending = PendingRead::new(&graphics, source, None).unwrap();
    unsafe { graphics.context.Flush() };
    let cpu = ByteBudget::new(1024).unwrap();
    let image = loop {
        if let Some(image) = pending.poll(&graphics, &cpu).unwrap() {
            break image;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(image.bytes(), bytes);
    // CopySubresourceRegion 移动交集从最新源读取，无同纹理原地覆盖。
    let destination = Arc::new(Texture::new(&graphics, 7, 3, false).unwrap());
    upload(&graphics, &destination, &[0; 84]);
    let source = D3D11_BOX {
        left: 0,
        top: 0,
        front: 0,
        right: 6,
        bottom: 3,
        back: 1,
    };
    unsafe {
        graphics.context.CopySubresourceRegion(
            &destination.native,
            0,
            1,
            0,
            0,
            &pending.staging.native,
            0,
            Some(&source),
        );
        graphics.context.Flush();
    }
    let moved = PendingRead::new(&graphics, destination, None).unwrap();
    unsafe { graphics.context.Flush() };
    let deadline = Instant::now() + Duration::from_secs(2);
    let moved = loop {
        if let Some(image) = moved.poll(&graphics, &cpu).unwrap() {
            break image;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    for y in 0..3 {
        assert_eq!(&moved.bytes()[y * 28..y * 28 + 4], &[0; 4]);
        assert_eq!(
            &moved.bytes()[y * 28 + 4..(y + 1) * 28],
            &bytes[y * 28..y * 28 + 24]
        );
    }
    // 已撤销但仍被消费者持有的租约不得继续占据冻结纹理预算。
    let epoch = crate::capture::lease::Epoch::new();
    let texture = Arc::new(Texture::new(&graphics, 32, 32, false).unwrap());
    let weak = Arc::downgrade(&texture);
    let lease = epoch.pin(TileMap::initial(texture));
    assert!(weak.upgrade().is_some());
    epoch.revoke();
    assert!(weak.upgrade().is_none());
    assert!(lease.get().is_err());
}
