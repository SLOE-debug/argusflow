//! 实际硬件计算着色器与 CPU 精确算法逐区域比对。
use super::*;

#[test]
#[ignore = "requires a hardware D3D11 FL11 compute device"]
fn hardware_shader_matches_exact_cpu_regions() {
    let mut differ = GpuDifference::new().unwrap();
    for format in [EvidencePixelFormat::Rgba8, EvidencePixelFormat::Bgrx8] {
        let bounds = InspectionRect {
            x: -100.0,
            y: -20.0,
            width: 67.0,
            height: 35.0,
        };
        let old = vec![0; 67 * 35 * 4];
        let previous = EvidenceFrame::new(bounds, 67, 35, format, old.clone()).unwrap();
        assert!(differ.compare(&previous, &previous).unwrap().is_empty());
        let mut pixels = old;
        for (x, y) in [(0, 0), (31, 31), (32, 32), (66, 34)] {
            let offset = (y * 67 + x) * 4;
            pixels[offset..offset + 4].copy_from_slice(&[17, 31, 3, 255]);
        }
        // BGRX 必须忽略保留通道变化；RGBA 必须保留 alpha 变化。
        pixels[15 * 4 + 3] = 25;
        let current = EvidenceFrame::new(bounds, 67, 35, format, pixels).unwrap();
        let expected = argusflow_capture::compare(
            argusflow_capture::PixelView::new(previous.pixels(), 67, 35, 67 * 4, format).unwrap(),
            argusflow_capture::PixelView::new(current.pixels(), 67, 35, 67 * 4, format).unwrap(),
            None,
        )
        .unwrap();
        let actual = differ.compare(&previous, &current).unwrap();
        let expected: Vec<_> = expected
            .regions()
            .iter()
            .map(|region| InspectionRect {
                x: bounds.x + f64::from(region.x),
                y: bounds.y + f64::from(region.y),
                width: region.width.into(),
                height: region.height.into(),
            })
            .collect();
        assert_eq!(actual, expected);
    }
}
