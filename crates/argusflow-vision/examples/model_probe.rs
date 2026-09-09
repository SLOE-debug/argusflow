//! 在不依赖 Python 的情况下验证两个官方模型的原生加载与推理。
use ndarray::Array4;
use ort::{ep, session::Session};
use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let root = PathBuf::from(
        arguments
            .next()
            .ok_or("usage: model_probe <deps-root> <cpu|cuda>")?,
    );
    let device = arguments.next().unwrap_or_else(|| "cpu".into());
    if !matches!(device.as_str(), "cpu" | "cuda") {
        return Err("device must be cpu or cuda".into());
    }
    let runtime = root.join("runtime").join(&device).join("onnxruntime.dll");
    ort::init_from(&runtime)?.commit();
    for tier in ["small", "medium"] {
        for kind in ["det", "rec"] {
            let started = std::time::Instant::now();
            let mut builder = Session::builder()?.with_intra_threads(4)?;
            if device == "cuda" {
                builder = builder.with_execution_providers([ep::CUDA::default()
                    .with_device_id(0)
                    .build()
                    .error_on_failure()])?;
            }
            let mut session = builder.commit_from_file(
                root.join("models")
                    .join(tier)
                    .join(kind)
                    .join("inference.onnx"),
            )?;
            let (height, width) = if kind == "det" { (96, 160) } else { (48, 320) };
            let tensor =
                ort::value::Tensor::from_array(Array4::<f32>::zeros((1, 3, height, width)))?;
            let output = session.run(ort::inputs![tensor])?;
            let (shape, values) = output[0].try_extract_tensor::<f32>()?;
            if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
                return Err("non-finite or empty inference output".into());
            }
            println!(
                "{device}/{tier}/{kind}: shape={shape:?}, elapsed={:?}",
                started.elapsed()
            );
        }
    }
    Ok(())
}
