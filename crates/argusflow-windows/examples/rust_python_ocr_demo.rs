//! 独立验证 Windows 捕获、Rust 像素封装、Named Pipe 传输与 Python PaddleOCR。

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use argusflow_core::{
    ElementMatcher, ElementRole, MatchOperator, PredicateValue, PropertyPredicate, QueryExpr,
    SelectorAttribute, UiQuery, WindowIdentity,
};
use argusflow_vision::{
    AppScene, AppWindowScene, CapturePolicy, OcrEngine, OcrProfile, OcrRequest, OcrResponse,
    PhysicalRect, SceneBuildOptions, VisualSceneBuilder, WindowDescriptor, WindowFrameSource,
    compile_vision_query, evaluate_vision_query, require_unique,
};
use argusflow_windows::capture::WindowsCaptureHost;
use serde_json::Value;
use uuid::Uuid;
use windows::Win32::{
    Foundation::HWND,
    System::Threading::CREATE_NO_WINDOW,
    UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
};

/// Python worker 完成模型加载前允许等待的时间。
const WORKER_STARTUP_TIMEOUT: Duration = Duration::from_secs(300);
/// 捕获第一张完整窗口帧的单次等待时间。
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);
/// 首轮探测使用的宽限期限，用于得到实际 OCR 耗时而不是提前取消。
const PROBE_OCR_TIMEOUT: Duration = Duration::from_secs(30);
/// 生产 small profile 当前使用的 OCR 截止时间。
const PRODUCTION_SMALL_TIMEOUT: Duration = Duration::from_secs(6);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let DemoArguments {
        window,
        project_root,
        query,
        exact,
    } = DemoArguments::parse()?;
    let project_root = project_root.unwrap_or(std::env::current_dir()?);
    let window = window.unwrap_or_else(foreground_window);

    println!("target hwnd={} pid={}", window.handle, window.process_id);

    let worker_started_at = Instant::now();
    let worker_launch = WorkerLaunch::from_project(&project_root)?;
    let mut worker_process = worker_launch.start()?;
    wait_for_worker_ready(&mut worker_process, WORKER_STARTUP_TIMEOUT).await?;
    println!(
        "python_worker_ready_ms={}",
        worker_started_at.elapsed().as_millis()
    );

    let engine = argusflow_vision::NamedPipeOcrEngine::new(
        worker_process.pipe_name.clone(),
        worker_process.session_token.clone(),
    );
    let handshake_started_at = Instant::now();
    let health = engine.refresh_health().await?;
    println!(
        "rust_python_handshake_ms={} lifecycle={:?} paddleocr={}",
        handshake_started_at.elapsed().as_millis(),
        health.lifecycle,
        health.paddleocr_version
    );

    let capture_host_started_at = Instant::now();
    let capture_host = WindowsCaptureHost::start()?;
    println!(
        "capture_host_start_ms={}",
        capture_host_started_at.elapsed().as_millis()
    );

    let frame_source = capture_host.frame_source();
    let open_started_at = Instant::now();
    let subscription = frame_source.open(window, CapturePolicy::default()).await?;
    println!("capture_open_ms={}", open_started_at.elapsed().as_millis());

    let frame_started_at = Instant::now();
    let frame = subscription.next(CAPTURE_TIMEOUT).await?;
    println!(
        "capture_first_frame_ms={} width={} height={} bytes={}",
        frame_started_at.elapsed().as_millis(),
        frame.width,
        frame.height,
        frame.pixels().len()
    );

    let probe_response = recognize_frame(
        &engine,
        window,
        frame.as_ref(),
        frame.bounds(),
        OcrProfile::small(),
        PROBE_OCR_TIMEOUT,
        "probe_30s",
    )
    .await?;
    print_items("probe_30s", &probe_response);

    let production_response = recognize_frame(
        &engine,
        window,
        frame.as_ref(),
        frame.bounds(),
        OcrProfile::small(),
        PRODUCTION_SMALL_TIMEOUT,
        "production_small_6s",
    )
    .await?;
    print_items("production_small_6s", &production_response);
    print_query_report(window, frame.as_ref(), &production_response, &query, exact)?;

    drop(subscription);
    drop(frame_source);
    capture_host.shutdown()?;
    Ok(())
}

/// 对同一捕获帧创建新的请求，并分别输出构造、端到端与 Python 推理耗时。
async fn recognize_frame(
    engine: &argusflow_vision::NamedPipeOcrEngine,
    window: WindowIdentity,
    frame: &argusflow_vision::CapturedFrame,
    roi: PhysicalRect,
    profile: OcrProfile,
    timeout: Duration,
    label: &str,
) -> Result<OcrResponse, Box<dyn Error>> {
    let request_started_at = Instant::now();
    let request = OcrRequest::from_frame(
        window,
        frame.frame_id,
        frame.topology_generation,
        frame,
        roi,
        profile,
        timeout,
    )?;
    println!(
        "{label}_request_build_ms={} body_bytes={}",
        request_started_at.elapsed().as_millis(),
        request.image.pixels().len()
    );

    let round_trip_started_at = Instant::now();
    let response = engine.recognize(request).await?;
    println!(
        "{label}_rust_round_trip_ms={} python_elapsed_ms={} items={} input={}x{} output={}x{} contrast={} sharpened={}",
        round_trip_started_at.elapsed().as_millis(),
        response.elapsed_ms,
        response.items.len(),
        response.preprocessing.input_width,
        response.preprocessing.input_height,
        response.preprocessing.output_width,
        response.preprocessing.output_height,
        response.preprocessing.contrast_enhanced,
        response.preprocessing.sharpened,
    );
    Ok(response)
}

/// 只打印有限条识别文本，既证明 OCR 返回了内容，也避免终端被完整窗口文本淹没。
fn print_items(label: &str, response: &OcrResponse) {
    for (index, item) in response.items.iter().take(12).enumerate() {
        println!(
            "{label}_item[{index}] confidence={:.3} text={:?}",
            item.confidence, item.raw_text
        );
    }
}

/// 使用与 Studio 相同的 AQL Vision 编译器执行匹配并输出求值指标。
fn print_query_report(
    window: WindowIdentity,
    frame: &argusflow_vision::CapturedFrame,
    response: &OcrResponse,
    query_text: &str,
    exact: bool,
) -> Result<(), Box<dyn Error>> {
    let mut builder = VisualSceneBuilder::new();
    let scene = builder.build(
        window,
        frame,
        std::slice::from_ref(response),
        &SceneBuildOptions::default(),
    )?;
    let operator = if exact {
        MatchOperator::Equal
    } else {
        MatchOperator::Contains
    };
    let query = UiQuery::new(QueryExpr::Match {
        matcher: ElementMatcher {
            role: ElementRole::Text,
            predicates: vec![PropertyPredicate {
                attribute: SelectorAttribute::Name,
                operator,
                value: PredicateValue::Text(query_text.to_owned()),
            }],
        },
    });
    let plan = compile_vision_query(&query)?;
    let app_scene = AppScene {
        process_id: window.process_id,
        windows: vec![AppWindowScene {
            window: WindowDescriptor {
                identity: window,
                owner_handle: None,
                z_order: 0,
                screen_bounds: frame.bounds(),
                foreground: true,
            },
            scene,
        }],
    };
    let result = evaluate_vision_query(&app_scene, &plan, query_text)?;
    println!(
        "aql_matches={} scanned_nodes={} exact_index_hits={} elapsed_us={}",
        result.matches.len(),
        result.metrics.scanned_nodes,
        result.metrics.exact_index_hits,
        result.metrics.elapsed_us,
    );
    require_unique(&result, query_text)?;
    Ok(())
}

/// demo 支持显式 HWND/PID；未传入时使用启动瞬间的前台窗口。
#[derive(Debug)]
struct DemoArguments {
    /// 明确指定的目标窗口，必须同时提供 HWND 和 PID。
    window: Option<WindowIdentity>,
    /// 仓库根目录；默认使用当前工作目录。
    project_root: Option<PathBuf>,
    /// 需要在 OCR 结果中精确查找的文字。
    query: String,
    /// 是否要求归一化后的 OCR 文字与查询完全相等。
    exact: bool,
}

impl DemoArguments {
    /// 解析最小命令行参数，不引入只为 demo 服务的参数库。
    fn parse() -> Result<Self, Box<dyn Error>> {
        let mut hwnd = None;
        let mut process_id = None;
        let mut project_root = None;
        let mut query = "网络结果".to_owned();
        let mut exact = false;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--hwnd" => {
                    hwnd = Some(next_value(&mut arguments, "--hwnd")?.parse::<u64>()?);
                }
                "--pid" => {
                    process_id = Some(next_value(&mut arguments, "--pid")?.parse::<u32>()?);
                }
                "--project-root" => {
                    project_root =
                        Some(PathBuf::from(next_value(&mut arguments, "--project-root")?));
                }
                "--query" => {
                    query = next_value(&mut arguments, "--query")?;
                }
                "--exact" => {
                    exact = true;
                }
                _ => return Err(format!("unknown argument: {argument}").into()),
            }
        }
        let window = match (hwnd, process_id) {
            (Some(handle), Some(process_id)) => Some(WindowIdentity { handle, process_id }),
            (None, None) => None,
            _ => return Err("--hwnd and --pid must be provided together".into()),
        };
        Ok(Self {
            window,
            project_root,
            query,
            exact,
        })
    }
}

/// 读取一个必需的命令行参数值。
fn next_value(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, Box<dyn Error>> {
    arguments
        .next()
        .ok_or_else(|| format!("missing value for {option}").into())
}

/// 读取前台窗口并冻结 HWND/PID 身份。
fn foreground_window() -> WindowIdentity {
    let window = unsafe { GetForegroundWindow() };
    let mut process_id = 0_u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    WindowIdentity {
        handle: window_handle(window),
        process_id,
    }
}

/// 把 Windows HWND 转换为领域层使用的不透明整数。
fn window_handle(window: HWND) -> u64 {
    window.0 as usize as u64
}

/// 独立 Python worker 的启动参数。
#[derive(Debug)]
struct WorkerLaunch {
    /// 仓库内专用 Conda Python 解释器。
    python: PathBuf,
    /// 可编辑安装的 worker 项目目录。
    worker_root: PathBuf,
    /// 当前 demo 独享的 Named Pipe。
    pipe_name: String,
    /// 当前 demo 独享的认证 token。
    session_token: String,
    /// worker 原子发布模型状态的临时文件。
    status_path: PathBuf,
}

impl WorkerLaunch {
    /// 从仓库约定的 worker 布局构造启动信息。
    fn from_project(project_root: &Path) -> Result<Self, Box<dyn Error>> {
        let worker_root = project_root.join("workers/argusflow-vision-worker");
        let python = worker_root.join(".conda/python.exe");
        if !python.is_file() {
            return Err(format!("vision worker Python was not found: {}", python.display()).into());
        }
        let run_id = Uuid::new_v4().simple().to_string();
        let runtime_root = project_root.join(".argusflow/dev/vision-demo");
        fs::create_dir_all(&runtime_root)?;
        Ok(Self {
            python,
            worker_root,
            pipe_name: format!(r"\\.\pipe\argusflow-vision-demo-{run_id}"),
            session_token: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
            status_path: runtime_root.join(format!("{run_id}.status.json")),
        })
    }

    /// 拉起与 Studio worker 隔离的 Python 进程。
    fn start(self) -> Result<WorkerProcess, Box<dyn Error>> {
        use std::os::windows::process::CommandExt;

        let child = Command::new(&self.python)
            .arg("-I")
            .arg("-m")
            .arg("argusflow_vision_worker")
            .arg("--pipe-name")
            .arg(&self.pipe_name)
            .arg("--session-token")
            .arg(&self.session_token)
            .arg("--status-file")
            .arg(&self.status_path)
            .current_dir(&self.worker_root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW.0)
            .spawn()?;
        Ok(WorkerProcess {
            child,
            pipe_name: self.pipe_name,
            session_token: self.session_token,
            status_path: self.status_path,
        })
    }
}

/// 确保成功和失败路径都会终止 demo 专属 worker。
#[derive(Debug)]
struct WorkerProcess {
    /// Python 子进程。
    child: Child,
    /// Rust 客户端连接使用的 Named Pipe。
    pipe_name: String,
    /// Rust 客户端握手使用的会话 token。
    session_token: String,
    /// 模型加载状态文件。
    status_path: PathBuf,
}

impl Drop for WorkerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.status_path);
    }
}

/// 等待 Python 完成两档模型预热，同时监测子进程提前退出和失败状态。
async fn wait_for_worker_ready(
    worker: &mut WorkerProcess,
    timeout: Duration,
) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = worker.child.try_wait()? {
            return Err(format!("vision worker exited during startup: {status}").into());
        }
        if let Ok(contents) = fs::read_to_string(&worker.status_path)
            && let Ok(status) = serde_json::from_str::<Value>(&contents)
        {
            match status.get("lifecycle").and_then(Value::as_str) {
                Some("ready") => return Ok(()),
                Some("failed") => {
                    let message = status
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown model startup failure");
                    return Err(format!("vision worker failed during startup: {message}").into());
                }
                _ => {}
            }
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "vision worker was not ready within {} seconds",
                timeout.as_secs()
            )
            .into());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
