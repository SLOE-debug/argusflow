//! 相同合成桌面负载下的区域路径与整屏读回对照，所有数据均由测试生成。
use super::*;
use windows::{
    Win32::{
        Foundation::FILETIME,
        Graphics::Direct3D11::*,
        System::Threading::{GetCurrentThread, GetThreadTimes},
    },
    core::BOOL,
};

struct GpuTimer {
    disjoint: ID3D11Query,
    start: ID3D11Query,
    end: ID3D11Query,
}
impl GpuTimer {
    fn begin(graphics: &Graphics) -> Self {
        let query = |kind| {
            let mut query = None;
            unsafe {
                graphics.device.CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: kind,
                        MiscFlags: 0,
                    },
                    Some(&mut query),
                )
            }
            .unwrap();
            query.unwrap()
        };
        let timer = Self {
            disjoint: query(D3D11_QUERY_TIMESTAMP_DISJOINT),
            start: query(D3D11_QUERY_TIMESTAMP),
            end: query(D3D11_QUERY_TIMESTAMP),
        };
        unsafe {
            graphics.context.Begin(&timer.disjoint);
            graphics.context.End(&timer.start);
        };
        timer
    }
    fn end(&self, graphics: &Graphics) {
        unsafe {
            graphics.context.End(&self.end);
            graphics.context.End(&self.disjoint);
            graphics.context.Flush();
        }
    }
    fn micros(&self, graphics: &Graphics) -> f64 {
        let mut timing = D3D11_QUERY_DATA_TIMESTAMP_DISJOINT {
            Frequency: 0,
            Disjoint: BOOL(1),
        };
        let mut first = 0u64;
        let mut last = 0u64;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            unsafe {
                graphics.context.GetData(
                    &self.disjoint,
                    Some((&mut timing as *mut D3D11_QUERY_DATA_TIMESTAMP_DISJOINT).cast()),
                    std::mem::size_of_val(&timing) as u32,
                    D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
                )
            }
            .unwrap();
            if timing.Frequency != 0 && !timing.Disjoint.as_bool() {
                break;
            }
            assert!(Instant::now() < deadline, "GPU timestamps unavailable");
            std::thread::sleep(Duration::from_millis(1));
        }
        unsafe {
            graphics
                .context
                .GetData(
                    &self.start,
                    Some((&mut first as *mut u64).cast()),
                    8,
                    D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
                )
                .unwrap();
            graphics
                .context
                .GetData(
                    &self.end,
                    Some((&mut last as *mut u64).cast()),
                    8,
                    D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
                )
                .unwrap();
        }
        (last - first) as f64 * 1_000_000.0 / timing.Frequency as f64
    }
}
fn cpu_100ns() -> u64 {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetThreadTimes(
            GetCurrentThread(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    }
    .unwrap();
    ((u64::from(kernel.dwHighDateTime) << 32) | u64::from(kernel.dwLowDateTime))
        + ((u64::from(user.dwHighDateTime) << 32) | u64::from(user.dwLowDateTime))
}
#[derive(Default)]
struct Report {
    wall: Vec<f64>,
    gpu: Vec<f64>,
    cpu_us: f64,
    pixels: u64,
    metadata: u64,
    peak_gpu: usize,
    peak_cpu: usize,
}
impl Report {
    fn print(&mut self, label: &str) {
        self.wall.sort_by(f64::total_cmp);
        self.gpu.sort_by(f64::total_cmp);
        let n = self.wall.len();
        println!(
            "{label}: samples={n} cpu_total_us={:.1} wall_p50_us={:.1} wall_p95_us={:.1} gpu_p50_us={:.1} gpu_p95_us={:.1} pixel_bytes={} metadata_bytes={} peak_gpu_bytes={} peak_cpu_bytes={}",
            self.cpu_us,
            self.wall[n / 2],
            self.wall[n * 95 / 100],
            self.gpu[n / 2],
            self.gpu[n * 95 / 100],
            self.pixels,
            self.metadata,
            self.peak_gpu,
            self.peak_cpu
        );
    }
}
#[test]
#[ignore = "explicit synthetic hardware performance comparison"]
fn region_readback_benchmark() {
    for full in [false, true] {
        let graphics = graphics();
        let before = Arc::new(Texture::new(&graphics, 1920, 1080, false).unwrap());
        let after = Arc::new(Texture::new(&graphics, 1920, 1080, false).unwrap());
        upload(&graphics, &before, &vec![0; 1920 * 1080 * 4]);
        upload(&graphics, &after, &vec![0; 1920 * 1080 * 4]);
        let cpu = ByteBudget::new(32 * 1024 * 1024).unwrap();
        let mut report = Report::default();
        let mut staging = None;
        for iteration in 0..100 {
            let cpu_start = cpu_100ns();
            let wall = Instant::now();
            let timer = GpuTimer::begin(&graphics);
            let changed = [(iteration % 2 + 1) as u8, 0, 0, 255].repeat(32 * 32);
            let box_ = D3D11_BOX {
                left: 64,
                top: 64,
                front: 0,
                right: 96,
                bottom: 96,
                back: 1,
            };
            unsafe {
                graphics.context.UpdateSubresource(
                    &after.native,
                    0,
                    Some(&box_),
                    changed.as_ptr().cast(),
                    32 * 4,
                    0,
                )
            };
            let pending_diff = if full {
                None
            } else {
                Some(
                    graphics
                        .difference
                        .submit(&graphics, &before, &after, &[[64, 64, 64, 64]])
                        .unwrap(),
                )
            };
            let texture = if full {
                after.clone()
            } else {
                let texture = Arc::new(Texture::new(&graphics, 32, 32, false).unwrap());
                unsafe {
                    graphics.context.CopySubresourceRegion(
                        &texture.native,
                        0,
                        0,
                        0,
                        0,
                        &after.native,
                        0,
                        Some(&box_),
                    );
                }
                texture
            };
            let pending = PendingRead::new(&graphics, texture, staging.take()).unwrap();
            timer.end(&graphics);
            let deadline = Instant::now() + Duration::from_secs(2);
            let image = loop {
                if let Some(image) = pending.poll(&graphics, &cpu).unwrap() {
                    break image;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            };
            if let Some(pending_diff) = pending_diff {
                let (changes, _) = pending_diff.poll(&graphics).unwrap().unwrap();
                assert_eq!(changes.changed_pixels, 1024);
                report.metadata += pending_diff.bytes();
            } else {
                // 连续整屏读回基线还需 CPU 逐像素发现变化，不使用哈希近似。
                let changed = image
                    .bytes()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|pixel| pixel[..3] != [0, 0, 0])
                    .count();
                assert_eq!(changed, 1024);
            }
            report.pixels += u64::from(image.width()) * u64::from(image.height()) * 4;
            report.gpu.push(timer.micros(&graphics));
            report.wall.push(wall.elapsed().as_secs_f64() * 1_000_000.0);
            report.cpu_us += (cpu_100ns() - cpu_start) as f64 / 10.0;
            staging = Some(pending.staging.clone());
        }
        report.peak_gpu = graphics.budget.peak();
        report.peak_cpu = cpu.peak();
        report.print(if full {
            "full_readback"
        } else {
            "region_readback"
        });
    }
}
