//! 显式运行的录制截图性能探针；不安装输入 Hook、不保存屏幕像素或窗口名称。

use argusflow_core::{InspectionProbe, WindowEvidenceCapture, WindowInspector};
use argusflow_windows::{capture::WindowsEventCapture, window::WindowsWindowInspector};
use std::{hint::black_box, time::Instant};

#[path = "support/capture_fixture.rs"]
mod fixture;
// 原 DIB 实现仅作为显式基准对照，不参与生产回退。
#[path = "../src/capture/evidence_geometry.rs"]
mod evidence_geometry;
#[path = "../src/capture/evidence_surface.rs"]
mod evidence_surface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let fixture = match args.as_slice() {
        [] => None,
        [flag, width, height] if flag == "--fixture" => Some(fixture::CaptureFixture::new(
            width.parse()?,
            height.parse()?,
        )?),
        _ => return Err("usage: recorder_capture_bench [--fixture WIDTH HEIGHT]".into()),
    };
    let capture = WindowsEventCapture::default();
    let mut baseline = None;
    let mut baseline_timings = Vec::new();
    let mut previous = None;
    let mut timings = Vec::new();
    let mut context_timings = Vec::new();
    let mut dimensions = (0, 0);
    // 首帧单独报告，前四帧预热；间隔采样覆盖桌面更新，不保存像素内容。
    for iteration in 0..104 {
        if let Some(fixture) = &fixture {
            fixture.paint(iteration)?;
        }
        let start = Instant::now();
        let context = match &fixture {
            Some(fixture) => WindowsWindowInspector.window_context(fixture.identity())?,
            None => WindowsWindowInspector.context(InspectionProbe::Focus)?,
        };
        let context_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        let frame = capture.capture(&context)?;
        let capture_ms = start.elapsed().as_secs_f64() * 1000.0;
        dimensions = (frame.width(), frame.height());
        if let Some(fixture) = &fixture {
            if !fixture.verify(&frame, iteration) {
                return Err(format!("stale/invalid dynamic frame at {iteration}").into());
            }
            if let Some((old, number)) = &previous {
                if !fixture.verify(old, *number) {
                    return Err("owned pixels overwritten".into());
                }
            }
            if std::env::var_os("ARGUSFLOW_BENCH_GDI").is_some() {
                // 同窗口、同尺寸、同一动态图案对照原 BitBlt 路径，包含两次身份校验。
                let start = Instant::now();
                evidence_geometry::validate(&context)?;
                let bounds = evidence_geometry::visible_bounds(context.bounds)?;
                let dc = evidence_surface::ScreenDc::acquire()?;
                if baseline.is_none() {
                    baseline = Some(evidence_surface::EvidenceSurface::new(
                        frame.width(),
                        frame.height(),
                    )?);
                }
                let old = baseline
                    .as_mut()
                    .ok_or("baseline unavailable")?
                    .capture(dc.0, bounds)?;
                drop(dc);
                evidence_geometry::validate(&context)?;
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                if !fixture.verify(&old, iteration) {
                    return Err("GDI baseline pixels invalid".into());
                }
                if iteration >= 4 {
                    baseline_timings.push(elapsed);
                }
            }
        }
        black_box(&frame);
        previous = Some((frame, iteration));
        if iteration == 0 {
            println!("cold_capture_ms={capture_ms:.3}");
        }
        if iteration >= 4 {
            timings.push(capture_ms);
            context_timings.push(context_ms);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    timings.sort_by(f64::total_cmp);
    context_timings.sort_by(f64::total_cmp);
    println!(
        "dimensions={}x{} samples=100 capture_min_ms={:.3} capture_median_ms={:.3} capture_p95_ms={:.3} capture_max_ms={:.3} context_median_ms={:.3}",
        dimensions.0,
        dimensions.1,
        timings[0],
        (timings[49] + timings[50]) / 2.0,
        timings[94],
        timings[99],
        (context_timings[49] + context_timings[50]) / 2.0
    );
    if !baseline_timings.is_empty() {
        baseline_timings.sort_by(f64::total_cmp);
        println!(
            "gdi_median_ms={:.3} gdi_p95_ms={:.3}",
            (baseline_timings[49] + baseline_timings[50]) / 2.0,
            baseline_timings[94]
        );
    }
    if fixture.is_some() {
        println!("dynamic_verified=104 owned_frame_verified=103");
    }
    Ok(())
}
