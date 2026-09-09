use super::*;
use std::collections::BTreeSet;
fn pixels(regions: &[PixelRect]) -> BTreeSet<(u32, u32)> {
    regions
        .iter()
        .flat_map(|r| {
            (r.y()..r.bottom()).flat_map(move |y| (r.x()..r.right()).map(move |x| (x, y)))
        })
        .collect()
}
#[test]
fn union_and_masks_preserve_exact_coverage() {
    let original = vec![
        PixelRect::new(0, 0, 4, 4).unwrap(),
        PixelRect::new(2, 2, 6, 3).unwrap(),
        PixelRect::new(0, 0, 4, 4).unwrap(),
    ];
    let normalized = normalize_regions(&original).unwrap();
    assert_eq!(pixels(&original), pixels(&normalized));
    let area: u64 = normalized.iter().map(|r| r.byte_len() / 4).sum();
    assert_eq!(area as usize, pixels(&normalized).len());
    let masks = [PixelRect::new(1, 1, 3, 2).unwrap()];
    let result = subtract_regions(&original, &masks).unwrap();
    assert_eq!(
        pixels(&result),
        pixels(&original)
            .difference(&pixels(&masks))
            .copied()
            .collect()
    );
}
#[test]
fn expansion_clips_and_keeps_separated_components() {
    let regions = [
        PixelRect::new(0, 0, 1, 1).unwrap(),
        PixelRect::new(99, 99, 1, 1).unwrap(),
    ];
    let result = reading_regions(&regions, 16, PixelRect::new(0, 0, 100, 100).unwrap()).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(pixels(&result).len(), 17 * 17 * 2);
}
#[test]
fn union_matches_bitmap_for_many_deterministic_shapes() {
    let mut seed = 29u64;
    for _ in 0..100 {
        let mut regions = Vec::new();
        for _ in 0..20 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = (seed % 16) as u32;
            let y = ((seed >> 8) % 16) as u32;
            regions.push(
                PixelRect::new(
                    x,
                    y,
                    1 + ((seed >> 16) % 8) as u32,
                    1 + ((seed >> 24) % 8) as u32,
                )
                .unwrap(),
            );
        }
        let normalized = normalize_regions(&regions).unwrap();
        assert_eq!(pixels(&normalized), pixels(&regions));
        assert_eq!(
            normalized.iter().map(|r| r.byte_len() / 4).sum::<u64>() as usize,
            pixels(&normalized).len()
        );
    }
}
