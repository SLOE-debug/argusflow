//! 只采集内存帧，不保存桌面图片、不注入任何用户输入。
use super::*;
use std::time::{Duration, Instant};
#[tokio::test]
#[ignore = "requires interactive DXGI desktop; no images saved"]
async fn full_screen_source_stops_threads_and_releases_cache_before_resume() {
    let mut prior = None;
    for _ in 0..2 {
        let source = DxgiFrameSource::start(FrameConfig::default()).unwrap();
        let start = Instant::now();
        let history = loop {
            let history = source.history().unwrap();
            if !history.is_empty()
                && history
                    .iter()
                    .all(|h| h.source.state == SourceState::Ready && !h.frames.is_empty())
            {
                break history;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "{history:?}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        if let Some(previous) = prior {
            assert_ne!(previous, source.clock().session);
        }
        prior = Some(source.clock().session);
        let pinned = history[0].frames.last().unwrap().clone();
        for screen in &history {
            let frame = screen.frames.last().unwrap();
            assert_eq!(
                (frame.image.width(), frame.image.height()),
                FrameConfig::default()
                    .size(screen.source.bounds.width(), screen.source.bounds.height())
            );
            assert!(screen.frames.len() <= 8);
            println!(
                "FULL_SCREEN source={} physical={}x{} image={}x{} startup_ms={}",
                screen.source.id.0,
                screen.source.bounds.width(),
                screen.source.bounds.height(),
                frame.image.width(),
                frame.image.height(),
                start.elapsed().as_millis()
            );
        }
        drop(history);
        let stop = Instant::now();
        source
            .shutdown(Operation::new(
                argusflow_core::OperationOptions::new(Duration::from_secs(2)).unwrap(),
            ))
            .await
            .unwrap();
        assert!(source.inner.threads.lock().unwrap().is_empty());
        assert!(source.inner.shared.store.lock().unwrap().sources.is_empty());
        assert!(source.history().is_err());
        assert!(!pinned.image.bytes().is_empty());
        drop(pinned);
        assert_eq!(source.inner.shared.cpu.used(), 0);
        println!(
            "FULL_SCREEN stopped_ms={} cache_bytes=0",
            stop.elapsed().as_millis()
        );
    }
}
