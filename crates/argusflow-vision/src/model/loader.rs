//! 校验官方模型包并创建固定设备的 ONNX Session。
use crate::OcrError as Failure;
use crate::{Device, OcrConfig};
use argusflow_core::{FailureKind, Operation};
use ort::{ep, session::Session};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

/// ONNX Runtime 动态库只允许进程级初始化一次，不接受后续偷偷换库。
static LOADED_RUNTIME: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

pub(crate) struct Models {
    pub(crate) detector: Session,
    pub(crate) recognizer: Session,
    pub(crate) dictionary: Vec<String>,
    pub(crate) threshold: f32,
    pub(crate) box_threshold: f32,
    pub(crate) unclip: f64,
}

#[derive(Deserialize)]
struct Manifest {
    artifacts: Vec<Artifact>,
}
#[derive(Deserialize)]
struct Artifact {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
struct DetectionConfig {
    #[serde(rename = "PostProcess")]
    post: DetectionPost,
}
#[derive(Deserialize)]
struct DetectionPost {
    name: String,
    thresh: f32,
    box_thresh: f32,
    unclip_ratio: f64,
}
#[derive(Deserialize)]
struct RecognitionConfig {
    #[serde(rename = "PostProcess")]
    post: RecognitionPost,
}
#[derive(Deserialize)]
struct RecognitionPost {
    name: String,
    character_dict: Vec<String>,
}

impl Models {
    pub(crate) fn load(config: &OcrConfig, operation: &Operation) -> Result<Self, Failure> {
        config.validate()?;
        let root = config
            .dependencies
            .canonicalize()
            .map_err(|error| unavailable("原生依赖目录不存在").with_source(error))?;
        let manifest: Manifest =
            serde_json::from_str(include_str!("../../../../scripts/native-deps.lock.json"))
                .map_err(|error| unavailable("模型锁文件无效").with_source(error))?;
        let prefix = format!("models/{}/", config.tier.directory());
        for artifact in manifest
            .artifacts
            .iter()
            .filter(|artifact| artifact.path.starts_with(&prefix))
        {
            operation.check("model_verify")?;
            let mut file = std::fs::File::open(root.join(&artifact.path)).map_err(|error| {
                unavailable("模型或配置文件缺失，请运行依赖准备脚本").with_source(error)
            })?;
            if file
                .metadata()
                .map_err(|error| unavailable("无法读取模型元数据").with_source(error))?
                .len()
                > 1_073_741_824
            {
                return Err(unavailable("模型文件大于 1 GiB 限制"));
            }
            let mut hash = Sha256::new();
            let mut buffer = vec![0u8; 1024 * 1024];
            loop {
                operation.check("model_verify")?;
                let length = file
                    .read(&mut buffer)
                    .map_err(|error| unavailable("模型读取失败").with_source(error))?;
                if length == 0 {
                    break;
                }
                hash.update(&buffer[..length]);
            }
            if format!("{:x}", hash.finalize()) != artifact.sha256 {
                return Err(unavailable("模型或配置 SHA-256 校验失败"));
            }
        }
        let model_root = root.join("models").join(config.tier.directory());
        let det_config: DetectionConfig = read_yaml(&model_root.join("det/inference.yml"))?;
        let rec_config: RecognitionConfig = read_yaml(&model_root.join("rec/inference.yml"))?;
        if det_config.post.name != "DBPostProcess" || rec_config.post.name != "CTCLabelDecode" {
            return Err(unavailable("模型后处理配置不受支持"));
        }
        let mut dictionary = vec![String::new()];
        dictionary.extend(rec_config.post.character_dict);
        dictionary.push(" ".into());
        let runtime_path = config
            .runtime_directory
            .clone()
            .unwrap_or_else(|| {
                root.join("runtime").join(match config.device {
                    Device::Cpu => "cpu",
                    Device::Cuda { .. } => "cuda",
                })
            })
            .join("onnxruntime.dll");
        initialize_runtime(&runtime_path)?;
        operation.check("model_load")?;
        let detector = session(config, &model_root.join("det/inference.onnx"))?;
        operation.check("model_load")?;
        let recognizer = session(config, &model_root.join("rec/inference.onnx"))?;
        Ok(Self {
            detector,
            recognizer,
            dictionary,
            threshold: det_config.post.thresh,
            box_threshold: det_config.post.box_thresh,
            unclip: det_config.post.unclip_ratio,
        })
    }
}

fn read_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Failure> {
    let bytes =
        std::fs::read(path).map_err(|error| unavailable("无法读取模型配置").with_source(error))?;
    serde_yaml::from_slice(&bytes)
        .map_err(|error| unavailable("模型配置解析失败").with_source(error))
}

fn initialize_runtime(path: &Path) -> Result<(), Failure> {
    let path = path
        .canonicalize()
        .map_err(|error| unavailable("ONNX Runtime DLL 缺失").with_source(error))?;
    let mut loaded = LOADED_RUNTIME
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| unavailable("ONNX Runtime 初始化锁损坏"))?;
    if let Some(existing) = loaded.as_ref() {
        // 同一 GPU Runtime 也能创建 CPU Session；但不能在进程里同时混装两份 ORT。
        if existing != &path {
            return Err(unavailable(
                "本进程已加载另一份 ONNX Runtime；CPU/CUDA 同进程使用时请统一配置 runtime 目录",
            ));
        }
        return Ok(());
    }
    ort::init_from(&path)
        .map_err(|error| unavailable("ONNX Runtime 加载失败").with_source(error))?
        .with_name("argusflow")
        .commit();
    *loaded = Some(path);
    Ok(())
}

fn session(config: &OcrConfig, path: &Path) -> Result<Session, Failure> {
    let mut builder = Session::builder()
        .map_err(native)?
        .with_intra_threads(config.cpu_threads)
        .map_err(native)?;
    if let Device::Cuda { device_id } = config.device {
        builder = builder
            .with_execution_providers([ep::CUDA::default()
                .with_device_id(device_id)
                .with_memory_limit(2 * 1024 * 1024 * 1024)
                .build()
                .error_on_failure()])
            .map_err(native)?;
    }
    builder.commit_from_file(path).map_err(native)
}

pub(crate) fn native(error: impl Into<ort::Error>) -> Failure {
    Failure::new(
        FailureKind::Native,
        "onnxruntime",
        "原生推理失败；请查看错误来源",
    )
    .with_source(error.into())
}
fn unavailable(message: &str) -> Failure {
    Failure::new(FailureKind::Unavailable, "ocr_models", message)
}

/// 当前推理 RunOptions 由取消守卫持有，取消后发送 ORT terminate。
pub(crate) type ActiveRun = Arc<Mutex<Option<Arc<ort::session::RunOptions>>>>;
