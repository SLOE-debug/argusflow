//! 只读复制用户指定录制到临时目录，复现真实并发回看命令。
use super::*;
#[tokio::test]
#[ignore = "需要ARGUSFLOW_VIDEO_REPRO指定已有录制；仅读取原件，在临时副本生成回看缓存"]
async fn overlapping_native_requests_wait_and_complete() {
    let source =
        PathBuf::from(std::env::var_os("ARGUSFLOW_VIDEO_REPRO").expect("set recording path"));
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path();
    std::fs::copy(source.join("session.json"), path.join("session.json")).unwrap();
    std::fs::copy(source.join("events.afr"), path.join("events.afr")).unwrap();
    std::fs::create_dir_all(path.join("video/000001")).unwrap();
    for name in ["session.json", "frames.jsonl", "screen.mp4"] {
        std::fs::copy(
            source.join("video/000001").join(name),
            path.join("video/000001").join(name),
        )
        .unwrap();
    }
    let header: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path.join("video/000001/session.json")).unwrap())
            .unwrap();
    let origin = header["qpc_origin"].as_i64().unwrap();
    let qpc = origin.to_string();
    let directory = path.to_string_lossy().into_owned();
    let started = std::time::Instant::now();
    let overview = super::super::timeline::recorder_video_timeline(directory.clone())
        .await
        .unwrap();
    assert_eq!(overview.segments.len(), 1);
    assert!(overview.duration_ms > 8000.0 && overview.duration_ms < 10000.0);
    assert!(
        overview
            .markers
            .iter()
            .any(|m| matches!(m.input, super::super::timeline::MarkerInput::Key { vk: 78 }))
    );
    assert!(
        overview
            .markers
            .iter()
            .any(|m| matches!(m.input, super::super::timeline::MarkerInput::Button { .. }))
    );
    println!(
        "TIMELINE_READ={:?}, MARKERS={}",
        started.elapsed(),
        overview.markers.len()
    );
    let started = std::time::Instant::now();
    let (first, second) = tokio::join!(
        recorder_video_frame(directory.clone(), qpc.clone(), 1, None, None),
        recorder_video_frame(directory.clone(), qpc, 1, None, None)
    );
    assert!(first.is_ok(), "{}", first.err().unwrap_or_default());
    assert!(second.is_ok(), "{}", second.err().unwrap_or_default());
    println!("BOTH_OK_IN={:?}", started.elapsed());
    // 从界面报告的 0.200 秒开始，连续请求递增播放位置。
    let session_origin = overview.origin_qpc.parse::<i64>().unwrap();
    let mut previous_sequence = 0;
    for ms in (200..=2000).step_by(120) {
        let playable = (ms as f64).max(overview.segments[0].start_ms);
        let target =
            session_origin + (playable * overview.frequency as f64 / 1000.0).round() as i64;
        let frame = recorder_video_frame(directory.clone(), target.to_string(), 0, None, None)
            .await
            .unwrap();
        assert!(frame.sequence >= previous_sequence);
        assert!(frame.at_qpc.parse::<i64>().unwrap() <= target);
        previous_sequence = frame.sequence;
    }
    assert!(previous_sequence > first.unwrap().sequence);
    let mut hashes = vec![];
    for offset in [
        7_560_000,
        14_099_999,
        16_184_607 - 1,
        19_184_607,
        68_690_000,
    ] {
        let qpc = (origin + offset).to_string();
        let started = std::time::Instant::now();
        let frame = recorder_video_frame(directory.clone(), qpc.clone(), 0, None, None)
            .await
            .unwrap();
        println!(
            "COLD offset={offset} seq={} elapsed={:?} bytes={}",
            frame.sequence,
            started.elapsed(),
            frame.image.bytes
        );
        let started = std::time::Instant::now();
        let cached = recorder_video_frame(directory.clone(), qpc, 0, None, None)
            .await
            .unwrap();
        println!("DISK_HIT elapsed={:?}", started.elapsed());
        if hashes.is_empty() {
            use image::ImageEncoder;
            let bytes = argusflow_recorder::read_attachment(path, &frame.image).unwrap();
            let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
            for filter in [
                image::codecs::png::FilterType::Sub,
                image::codecs::png::FilterType::NoFilter,
            ] {
                let started = std::time::Instant::now();
                let mut output = vec![];
                image::codecs::png::PngEncoder::new_with_quality(
                    &mut output,
                    image::codecs::png::CompressionType::Fast,
                    filter,
                )
                .write_image(
                    &decoded,
                    decoded.width(),
                    decoded.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .unwrap();
                println!(
                    "ENCODE {filter:?} {:?} {} bytes",
                    started.elapsed(),
                    output.len()
                );
            }
        }
        assert_eq!(frame.image.hash, cached.image.hash);
        hashes.push(frame.image.hash);
    }
    assert_ne!(hashes[1], hashes[2]);
    assert_ne!(hashes[2], hashes[3]);
    // 第10步必须有自己的目标画面，不能借用上一键或打开窗口后的结果。
    let before = recorder_video_frame(
        directory.clone(),
        (origin + 40_973_234).to_string(),
        -1,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(before.sequence, 56);
    assert!(before.presented_qpc.parse::<i64>().unwrap() < origin + 40_973_234);
    std::fs::write(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.cache/click-before-56.png"),
        argusflow_recorder::read_attachment(path, &before.image).unwrap(),
    )
    .unwrap();
    for (baseline, anchor, earliest, expected) in [
        (40_973_234, 41_565_042, 48_792_194, 73),
        (67_986_807, 68_605_935, 71_605_935, 112),
    ] {
        let started = std::time::Instant::now();
        let frame = recorder_video_frame(
            directory.clone(),
            (origin + anchor + 20_000_000).to_string(),
            0,
            Some((origin + baseline).to_string()),
            Some((origin + earliest).to_string()),
        )
        .await
        .unwrap();
        assert_eq!(frame.sequence, expected);
        println!(
            "CLICK_RESULT seq={} elapsed={:?} analysis={}",
            frame.sequence,
            started.elapsed(),
            frame.at_qpc
        );
        let bytes = argusflow_recorder::read_attachment(path, &frame.image).unwrap();
        std::fs::write(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../.cache/pixel-result-{}.png", frame.sequence)),
            bytes,
        )
        .unwrap();
        let started = std::time::Instant::now();
        let cached = recorder_video_frame(
            directory.clone(),
            (origin + anchor + 20_000_000).to_string(),
            0,
            Some((origin + baseline).to_string()),
            Some((origin + earliest).to_string()),
        )
        .await
        .unwrap();
        assert_eq!(cached.sequence, frame.sequence);
        println!("ANALYSIS_CACHE_HIT={:?}", started.elapsed());
    }
    for (anchor, target, expected) in [
        (12_086_909, 14_099_998, 23),
        (14_100_032, 16_184_606, 26),
        (16_184_607, 19_184_607, 33),
    ] {
        let frame = recorder_video_frame(
            directory.clone(),
            (origin + target).to_string(),
            0,
            Some((origin + anchor).to_string()),
            None,
        )
        .await
        .unwrap();
        println!(
            "TEXT_RESULT seq={} analysis={}",
            frame.sequence, frame.at_qpc
        );
        assert_eq!(frame.sequence, expected);
        let bytes = argusflow_recorder::read_attachment(path, &frame.image).unwrap();
        std::fs::write(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../.cache/pixel-text-{}.png", frame.sequence)),
            bytes,
        )
        .unwrap();
    }
}
