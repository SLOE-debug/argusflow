//! 合成输入日志测量，不监听桌面，不读用户窗口。
#[allow(dead_code)]
mod fixtures;
use argusflow_recorder::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let mut session = fixtures::session();
    session.qpc_frequency = 1_000_000_000;
    let mut writer = RecordingWriter::create(&temp.path().join("recording"), &session, 500, 4)?;
    let (tx, rx) = mpsc::sync_channel(8192);
    let queued = Arc::new(AtomicUsize::new(0));
    let queue = queued.clone();
    let peak = Arc::new(AtomicUsize::new(0));
    let max = peak.clone();
    let started = Instant::now();
    let origin = started;
    let producer = std::thread::spawn(move || {
        for index in 0..20_000 {
            let count = queue.fetch_add(1, Ordering::AcqRel) + 1;
            max.fetch_max(count, Ordering::Relaxed);
            let mut event = fixtures::button(index % 2 == 0, origin.elapsed().as_nanos() as i64);
            event.sequence = index + 1;
            tx.send(event).unwrap();
            // 100事件一批，每批间隔10ms：压力形态明确，不冒充真实键鼠负载。
            if index % 100 == 99 {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    });
    let mut latency = vec![];
    let mut sync = Instant::now();
    while let Ok(event) = rx.recv() {
        queued.fetch_sub(1, Ordering::AcqRel);
        let record = writer.raw(event, started.elapsed().as_nanos() as i64)?;
        latency.push((started.elapsed().as_nanos() as i64 - event.qpc) as u64);
        writer.normalize(&record, started.elapsed().as_nanos() as i64)?;
        if sync.elapsed() > Duration::from_millis(250) {
            writer.sync(started.elapsed().as_nanos() as i64)?;
            sync = Instant::now();
        }
    }
    producer.join().unwrap();
    writer.sync(started.elapsed().as_nanos() as i64)?;
    latency.sort_unstable();
    let size = std::fs::metadata(temp.path().join("recording/events.afr"))?.len();
    println!(
        "{}",
        serde_json::json!({"condition":"debug build;20,000 synthetic events;100-event bursts/10ms;8192 queue;250ms sync;no native capture/OCR","events":latency.len(),"elapsed_ms":started.elapsed().as_millis(),"log_ack_us":{"p50":latency[latency.len()/2]/1000,"p95":latency[latency.len()*95/100]/1000,"p99":latency[latency.len()*99/100]/1000,"max":latency.last().unwrap()/1000},"queue_peak":peak.load(Ordering::Acquire),"known_lost":0,"journal_bytes":size,"images":0,"visual_latency":"not measured","gpu":"not used"})
    );
    Ok(())
}
