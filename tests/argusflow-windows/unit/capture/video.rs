//! 视频时钟、持久化约束和原生样本租约回归。
use super::*;
use crate::capture::clock::Clock;
use argusflow_capture_contracts::ClockDomain;
use std::{sync::mpsc, time::Duration};
use windows::{Win32::Graphics::Dxgi::*, core::Interface};

#[test]
#[ignore = "ARGUSFLOW_VIDEO_REPRO指定已有录制；只读比较连续解码、回退和随机seek，不操作桌面"]
fn reused_decoder_matches_independent_seeks() {
    let path = std::path::PathBuf::from(std::env::var_os("ARGUSFLOW_VIDEO_REPRO").unwrap())
        .join("video/000001/screen.mp4");
    let mut decoder = VideoDecoder::open(&path).unwrap();
    for pts in [
        13_890_000, 15_880_000, 18_220_000, 7_220_000, 68_690_000, 13_890_000,
    ] {
        let reused = decoder.read(pts).unwrap();
        let fresh = decode_video_frame(&path, pts).unwrap();
        assert_eq!(reused.rgba, fresh.rgba, "PTS {pts}");
    }
}

#[test]
fn large_qpc_maps_thirty_minutes_without_float_drift() {
    let origin = 9_007_199_254_740_993;
    let clock = Clock(ClockDomain {
        session: 1,
        origin,
        frequency: 10_000_000,
    });
    assert_eq!(timing::pts(origin + 18_000_000_000, clock), 18_000_000_000);
    assert_eq!(timing::pts(origin + 1234567, clock), 1234567);
    assert_eq!(timing::pts(origin - 1, clock), 0);
}

#[test]
fn invalid_time_is_rejected_instead_of_fabricating_duration() {
    assert_eq!(timing::duration(1, 31).unwrap(), 30);
    assert!(timing::duration(31, 31).is_err());
    assert!(timing::duration(31, 1).is_err());
}

#[test]
fn quantizing_boundaries_does_not_accumulate_thirty_minute_drift() {
    let mut previous = 0;
    let mut total = 0;
    for frame in 1..=54000i64 {
        let actual = frame * 18_000_000_000 / 54000;
        let encoded = timing::media_pts(actual);
        assert!((encoded - actual).abs() <= 5000);
        total += timing::duration(previous, encoded).unwrap();
        previous = encoded;
    }
    assert_eq!(total, 18_000_000_000);
}

#[test]
fn quota_rejects_without_deleting_existing_recording() {
    let path = std::env::temp_dir().join(format!(
        "argusflow-video-quota-{}-{}",
        std::process::id(),
        timing::qpc().unwrap()
    ));
    std::fs::create_dir(&path).unwrap();
    let video = path.join("screen.mp4");
    std::fs::write(&video, [7u8; 32]).unwrap();
    assert!(storage::StorageGuard::new(&path, 32).is_err());
    assert_eq!(std::fs::read(&video).unwrap(), [7u8; 32]);
    std::fs::remove_file(video).unwrap();
    std::fs::remove_dir(path).unwrap();
}

#[test]
fn journal_never_overwrites_existing_evidence() {
    let path = std::env::temp_dir().join(format!(
        "argusflow-video-journal-{}-{}.jsonl",
        std::process::id(),
        timing::qpc().unwrap()
    ));
    let mut log = journal::Journal::new(&path).unwrap();
    log.write(&serde_json::json!({"id":1})).unwrap();
    log.sync().unwrap();
    assert!(journal::Journal::new(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"id\":1}\n");
    drop(log);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[ignore = "需要D3D11 GPU和Media Foundation；不采集桌面"]
fn texture_returns_only_after_last_sample_reference() {
    let _runtime = runtime::Runtime::new().unwrap();
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1().unwrap() };
    let adapter = unsafe { factory.EnumAdapters1(0).unwrap() };
    let graphics = graphics::Graphics::new(&adapter, 64, 64, 30).unwrap();
    let texture = graphics.texture().unwrap();
    let (returned, available) = mpsc::sync_channel(1);
    let callback = sample::recycler(returned);
    let sample = sample::sample(&texture, 0, 333333, &callback).unwrap();
    let retained = sample.clone();
    drop(sample);
    assert!(available.recv_timeout(Duration::from_millis(30)).is_err());
    drop(retained);
    let recycled = available.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(texture.as_raw(), recycled.as_raw());
    assert!(available.try_recv().is_err());
}

#[test]
#[ignore = "需要硬件H264编码；合成GPU图案，不采集桌面；验证30分钟媒体时间轴而非30分钟墙钟"]
fn hardware_encoder_persists_thirty_minute_timeline() {
    use journal::{FrameEntry, Message, PendingFrame};
    use windows::Win32::Graphics::{Direct3D11::*, Dxgi::Common::*};
    let _runtime = runtime::Runtime::new().unwrap();
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1().unwrap() };
    let adapter = unsafe { factory.EnumAdapters1(0).unwrap() };
    let graphics = graphics::Graphics::new(&adapter, 640, 360, 30).unwrap();
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.cache")
        .join(format!("video-synthetic-{}", timing::qpc().unwrap()));
    std::fs::create_dir(&directory).unwrap();
    let mut encoder = encoder::Encoder::new(
        &graphics.device,
        &directory.join("screen.mp4"),
        640,
        360,
        30,
        2_000_000,
    )
    .unwrap();
    let (sender, receiver) = mpsc::sync_channel(4);
    let (returned, _available) = mpsc::sync_channel(4);
    // 原始BGRA图案只属于测试。真实录制不会构造CPU像素数组。
    for (index, color) in [[0u8, 0, 255, 255], [0, 255, 0, 255], [255, 0, 0, 255]]
        .into_iter()
        .enumerate()
    {
        let mut bytes = color.repeat(640 * 360);
        // 不对称角标校验顶向下行顺序与H264填充区域裁剪。
        for y in 0..64 {
            for x in 0..64 {
                bytes[(y * 640 + x) * 4..][..4].copy_from_slice(&[255, 255, 255, 255]);
                bytes[((359 - y) * 640 + 639 - x) * 4..][..4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: 640,
            Height: 360,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            ..Default::default()
        };
        let mut source = None;
        unsafe {
            graphics
                .device
                .CreateTexture2D(
                    &desc,
                    Some(&D3D11_SUBRESOURCE_DATA {
                        pSysMem: bytes.as_ptr().cast(),
                        SysMemPitch: 640 * 4,
                        SysMemSlicePitch: 0,
                    }),
                    Some(&mut source),
                )
                .unwrap();
        }
        let output = graphics.texture().unwrap();
        graphics.convert(&source.unwrap(), &output).unwrap();
        sender
            .send(Message::Frame(PendingFrame {
                texture: output,
                entry: FrameEntry {
                    sequence: index as u64 + 1,
                    presented_qpc: index as i64 * 6_000_000_000,
                    acquired_qpc: index as i64 * 6_000_000_000,
                    pts_100ns: index as i64 * 6_000_000_000,
                    accumulated: 1,
                    repeated: false,
                },
            }))
            .unwrap();
    }
    sender.send(Message::End(18_000_000_000)).unwrap();
    drop(sender);
    writer::encode(&mut encoder, &receiver, returned, &directory, 30).unwrap();
    let records: Vec<serde_json::Value> = std::fs::read_to_string(directory.join("frames.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 3);
    assert!(
        records
            .iter()
            .all(|r| r["duration_100ns"] == 6_000_000_000i64)
    );
    assert!(
        std::fs::metadata(directory.join("screen.mp4"))
            .unwrap()
            .len()
            > 1024
    );
    println!("SYNTHETIC_VIDEO={}", directory.display());
    for (index, channel) in [0usize, 1, 2].into_iter().enumerate() {
        let decoded =
            decode_video_frame(&directory.join("screen.mp4"), index as i64 * 6_000_000_000)
                .unwrap();
        assert_eq!((decoded.width, decoded.height), (640, 360));
        let pixel = &decoded.rgba[(180 * 640 + 320) * 4..][..4];
        assert!(pixel[channel] > 200, "{pixel:?}");
        assert!(pixel[(channel + 1) % 3] < 50, "{pixel:?}");
        assert!(
            decoded.rgba[(32 * 640 + 32) * 4..][..3]
                .iter()
                .all(|v| *v > 220)
        );
        assert!(
            decoded.rgba[(327 * 640 + 607) * 4..][..3]
                .iter()
                .all(|v| *v < 30)
        );
    }
}
