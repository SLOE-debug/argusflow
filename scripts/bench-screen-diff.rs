//! rustc -O scripts/bench-screen-diff.rs -o target/bench-screen-diff.exe
//! 合成画面基准，不访问桌面；使用平台基线 ISA，不传 target-cpu=native。
#[path = "../crates/argusflow-recorder/src/screen_diff_kernel.rs"]
mod kernel;
use std::{hint::black_box, time::Instant};
fn main() {
    for (width, height) in [(1920, 1080), (3840, 2160)] {
        let original = vec![42; width * height * 4];
        for scenario in ["static", "one_pixel", "small_region", "full_change"] {
            let mut current = original.clone();
            match scenario {
                "one_pixel" => current[(height / 2 * width + width / 2) * 4] = 99,
                "small_region" => {
                    for row in height / 2..height / 2 + 64 {
                        current[(row * width + width / 2) * 4..(row * width + width / 2 + 128) * 4]
                            .fill(99);
                    }
                }
                "full_change" => current.fill(99),
                _ => {}
            }
            let expected = kernel::count(&original, &current, width, height, false);
            for vector in [false, true] {
                let mut samples = Vec::new();
                for round in 0..110 {
                    let start = Instant::now();
                    let actual = kernel::count(
                        black_box(&original),
                        black_box(&current),
                        width,
                        height,
                        black_box(vector),
                    );
                    let micros = start.elapsed().as_secs_f64() * 1e6;
                    assert_eq!(actual, expected);
                    if round >= 10 {
                        samples.push(micros);
                    }
                }
                samples.sort_by(f64::total_cmp);
                println!(
                    "{width}x{height} {scenario} {} median_us={:.2} p95_us={:.2} tiles={expected}",
                    if vector { "SSE2" } else { "slice_eq" },
                    samples[50],
                    samples[95]
                );
            }
        }
    }
}
