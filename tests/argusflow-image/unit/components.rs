use super::*;
use argusflow_core::OperationOptions;

/// 独立的逐像素栈遍历作为参考，不复用生产位图或邻接实现。
fn reference(mut pixels: Vec<bool>, width: usize) -> Vec<(u32, u32, u32, u32, usize)> {
    let height = pixels.len() / width;
    let mut result = Vec::new();
    for seed in 0..pixels.len() {
        if !pixels[seed] {
            continue;
        }
        let mut stack = vec![(seed % width, seed / width)];
        let (mut left, mut top, mut right, mut bottom, mut area) = (width, height, 0, 0, 0);
        while let Some((x, y)) = stack.pop() {
            if !pixels[y * width + x] {
                continue;
            }
            pixels[y * width + x] = false;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
            area += 1;
            if x > 0 {
                stack.push((x - 1, y));
            }
            if y > 0 {
                stack.push((x, y - 1));
            }
            if x + 1 < width {
                stack.push((x + 1, y));
            }
            if y + 1 < height {
                stack.push((x, y + 1));
            }
        }
        result.push((
            left as u32,
            top as u32,
            (right - left + 1) as u32,
            (bottom - top + 1) as u32,
            area,
        ));
    }
    result.sort_unstable();
    result
}

#[test]
fn exact_components_match_reference_across_density_and_word_boundaries() {
    let operation = Operation::new(OperationOptions::default());
    let mut random = 123456789_u64;
    for width in [1, 4, 63, 64, 65, 257] {
        for density in [0, 1, 10, 50, 99, 100] {
            for _ in 0..8 {
                let mask: Vec<_> = (0..width * 13)
                    .map(|_| {
                        random ^= random << 13;
                        random ^= random >> 7;
                        random ^= random << 17;
                        random % 100 < density
                    })
                    .collect();
                let a = vec![0; mask.len() * 3];
                let b: Vec<_> = mask.iter().flat_map(|&v| [u8::from(v); 3]).collect();
                let mut actual: Vec<_> = changed_regions(
                    ImageView::triples(width as u32, 13, &a).unwrap(),
                    ImageView::triples(width as u32, 13, &b).unwrap(),
                    DifferencePolicy::default(),
                    &operation,
                )
                .unwrap()
                .into_iter()
                .map(|r| {
                    (
                        r.bounds.x(),
                        r.bounds.y(),
                        r.bounds.width(),
                        r.bounds.height(),
                        r.pixels,
                    )
                })
                .collect();
                actual.sort_unstable();
                assert_eq!(actual, reference(mask, width));
            }
        }
    }
}
