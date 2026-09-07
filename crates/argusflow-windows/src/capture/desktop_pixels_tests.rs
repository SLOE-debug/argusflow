//! 用非方形像素矩阵验证旋转、跨屏拼接、行填充与几何拒绝。

use super::desktop_pixels::{OutputCrop, Rotation};
use argusflow_core::InspectionRect;
use windows::Win32::Foundation::RECT;

fn bounds(x: f64, y: f64, width: f64, height: f64) -> InspectionRect {
    InspectionRect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn all_rotations_preserve_non_square_pixels_and_padding() {
    // 纹理两行三列，每行附加一个像素宽度的 padding。
    let source: Vec<u8> = [1, 2, 3, 99, 4, 5, 6, 99]
        .into_iter()
        .flat_map(|n| [n; 4])
        .collect();
    for (rotation, width, height, expected) in [
        (Rotation::Identity, 3, 2, vec![1, 2, 3, 4, 5, 6]),
        (Rotation::Clockwise90, 2, 3, vec![4, 1, 5, 2, 6, 3]),
        (Rotation::Clockwise180, 3, 2, vec![6, 5, 4, 3, 2, 1]),
        (Rotation::Clockwise270, 2, 3, vec![3, 6, 2, 5, 1, 4]),
    ] {
        let rectangle = bounds(-5.0, -3.0, f64::from(width), f64::from(height));
        let output = RECT {
            left: -5,
            top: -3,
            right: -5 + width,
            bottom: -3 + height,
        };
        let crop = OutputCrop::new(rectangle, output, rotation).unwrap();
        let mut target = vec![0; 24];
        crop.copy(&source, 16, 3, 2, &mut target).unwrap();
        assert_eq!(
            target
                .chunks_exact(4)
                .map(|pixel| pixel[0])
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn negative_origin_and_monitor_gap_are_composed_without_shifting_pixels() {
    let target_bounds = bounds(-1.0, 1.0, 5.0, 1.0);
    let left = OutputCrop::new(
        target_bounds,
        RECT {
            left: -2,
            top: 0,
            right: 0,
            bottom: 2,
        },
        Rotation::Identity,
    )
    .unwrap();
    let right = OutputCrop::new(
        target_bounds,
        RECT {
            left: 2,
            top: 0,
            right: 4,
            bottom: 2,
        },
        Rotation::Identity,
    )
    .unwrap();
    let source: Vec<_> = [1, 2, 3, 4].into_iter().flat_map(|n| [n; 4]).collect();
    let mut target = vec![0; 20];
    left.copy(&source, 8, 2, 2, &mut target).unwrap();
    right.copy(&source, 8, 2, 2, &mut target).unwrap();
    assert_eq!(
        target
            .chunks_exact(4)
            .map(|pixel| pixel[0])
            .collect::<Vec<_>>(),
        [4, 0, 0, 3, 4]
    );
    assert!(left.copy(&source, 7, 2, 2, &mut target).is_err());
    assert!(left.copy(&source[..8], 8, 2, 2, &mut target).is_err());
    assert!(left.copy(&source, 8, 1, 4, &mut target).is_err());
    assert!(left.copy(&source, 8, 2, 2, &mut target[..16]).is_err());
    assert!(
        OutputCrop::new(
            bounds(10.0, 10.0, 2.0, 2.0),
            RECT {
                left: 0,
                top: 0,
                right: 2,
                bottom: 2
            },
            Rotation::Identity
        )
        .is_none()
    );
}

#[test]
fn gpu_region_and_local_rotation_match_full_texture_crop() {
    let source: Vec<u8> = (1..=12).flat_map(|n| [n; 4]).collect();
    for (rotation, output_width, output_height) in [
        (Rotation::Identity, 4, 3),
        (Rotation::Clockwise90, 3, 4),
        (Rotation::Clockwise180, 4, 3),
        (Rotation::Clockwise270, 3, 4),
    ] {
        let crop = OutputCrop::new(
            bounds(-4.0, -2.0, 2.0, 2.0),
            RECT {
                left: -5,
                top: -3,
                right: -5 + output_width,
                bottom: -3 + output_height,
            },
            rotation,
        )
        .unwrap();
        let region = crop.texture_region();
        let mut gpu_crop = Vec::new();
        for row in region.top..region.bottom {
            let start = (row * 4 + region.left) as usize * 4;
            gpu_crop.extend_from_slice(
                &source[start..start + (region.right - region.left) as usize * 4],
            );
        }
        let mut expected = vec![0; 16];
        let mut actual = vec![0; 16];
        crop.copy(&source, 16, 4, 3, &mut expected).unwrap();
        crop.copy_cropped(
            &gpu_crop,
            (region.right - region.left) as usize * 4,
            region.right - region.left,
            region.bottom - region.top,
            &mut actual,
        )
        .unwrap();
        assert_eq!(actual, expected);
    }
}
