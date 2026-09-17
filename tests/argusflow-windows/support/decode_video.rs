//! 指定已有视频与精确PTS的独立复现进程；由调用者提供超时监督。
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("missing video path")?;
    let times = std::env::args().nth(2).ok_or("missing pts")?;
    let started = std::time::Instant::now();
    let mut decoder = argusflow_windows::VideoDecoder::open(std::path::Path::new(&path))?;
    println!("open in {:?}", started.elapsed());
    for time in times.split(',') {
        let pts = time.parse()?;
        eprintln!("decode start: pts={pts}");
        let started = std::time::Instant::now();
        let frame = decoder.read(pts)?;
        println!(
            "decoded {}x{} in {:?}",
            frame.width,
            frame.height,
            started.elapsed()
        );
        if let Some(output) = std::env::args_os().nth(3) {
            std::fs::write(output, &frame.rgba)?;
        }
    }
    Ok(())
}
