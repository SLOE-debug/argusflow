//! 一张图片的检测、逐区域识别与结果组装。
use super::{detection, preprocessing, recognition};
use crate::OcrError as Failure;
use crate::{
    ImageInput, OcrConfig, OcrResult, TextBlock,
    model::{ActiveRun, Models, native},
};
use argusflow_core::{FailureKind, Operation};
use ndarray::Array4;
use ort::{
    session::{RunOptions, Session},
    value::Tensor,
};
use std::sync::Arc;

pub(crate) fn recognize(
    models: &mut Models,
    input: ImageInput,
    config: &OcrConfig,
    operation: &Operation,
    active: &ActiveRun,
) -> Result<OcrResult, Failure> {
    let image = input.decode(config, operation)?;
    let det_input = preprocessing::detection(&image, config, operation)?;
    let (shape, values) = run(&mut models.detector, det_input, operation, active)?;
    if shape.len() != 4 || shape[0] != 1 || shape[1] != 1 {
        return Err(protocol("检测模型输出维度错误"));
    }
    let polygons = detection::boxes(
        &values,
        shape[3],
        shape[2],
        image.dimensions(),
        config,
        operation,
        detection::Parameters {
            threshold: models.threshold,
            box_threshold: models.box_threshold,
            unclip: models.unclip,
        },
    )?;
    let mut blocks = Vec::with_capacity(polygons.len());
    for polygon in polygons {
        operation.check("ocr_region")?;
        let cropped = preprocessing::crop(&image, &polygon, operation)?;
        let rec_input = preprocessing::recognition(&cropped, operation)?;
        let (shape, values) = run(&mut models.recognizer, rec_input, operation, active)?;
        if shape.len() != 3 || shape[0] != 1 {
            return Err(protocol("识别模型输出维度错误"));
        }
        let (text, confidence) = recognition::ctc(&values, shape[1], shape[2], &models.dictionary)?;
        if !text.is_empty() && confidence >= config.minimum_confidence {
            blocks.push(TextBlock {
                text,
                confidence,
                polygon,
            });
        }
    }
    operation.check("ocr_complete")?;
    Ok(OcrResult {
        blocks,
        width: image.width(),
        height: image.height(),
    })
}

fn run(
    session: &mut Session,
    input: Array4<f32>,
    operation: &Operation,
    active: &ActiveRun,
) -> Result<(Vec<usize>, Vec<f32>), Failure> {
    let options = Arc::new(RunOptions::new().map_err(native)?);
    *active.lock().unwrap_or_else(|p| p.into_inner()) = Some(options.clone());
    // 先发布 RunOptions 再复验取消，避免守卫取消与推理启动之间丢失信号。
    operation.check("inference_start")?;
    let tensor = Tensor::from_array(input).map_err(native)?;
    let result = (|| {
        let outputs = session
            .run_with_options(ort::inputs![tensor], &options)
            .map_err(native)?;
        operation.check("inference_complete")?;
        let (shape, values) = outputs[0].try_extract_tensor::<f32>().map_err(native)?;
        let shape = shape
            .iter()
            .map(|dimension| usize::try_from(*dimension).map_err(|_| protocol("模型输出尺寸无效")))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((shape, values.to_vec()))
    })();
    *active.lock().unwrap_or_else(|p| p.into_inner()) = None;
    result
}
fn protocol(message: &str) -> Failure {
    Failure::new(FailureKind::Protocol, "ocr_pipeline", message)
}
