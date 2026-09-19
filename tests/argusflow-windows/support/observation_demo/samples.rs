//! 独立定时采样。前帧只能取早于事件的历史记录，不能事后伪造。
use super::{
    Result, clipboard,
    observe::write,
    wechat_support::{capture, desktop::Desktop, frame::Frame, observation::Observer},
};
use argusflow_aql::{Attribute, Bindings, Value, compile};
use argusflow_core::{Operation, OperationOptions};
use argusflow_vision::{OcrConfig, OcrEngine};
use argusflow_windows::{InputService, UiaRuntime, WindowLocator};
use serde_json::{Value as Json, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
pub fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_millis() as u64
}
pub struct Sampler {
    directory: PathBuf,
    file: std::fs::File,
    runtime: UiaRuntime,
    input: InputService,
    engine: OcrEngine,
    observer: Observer,
    previous: HashMap<isize, Frame>,
    previous_images: HashMap<isize, String>,
    image_bytes: usize,
    sequence: u32,
    clipboard: Option<String>,
    index: usize,
    screen: super::screen::Screen,
    clipboard_reader: argusflow_windows::ClipboardReader,
    isolated: bool,
}
impl Sampler {
    pub async fn start(directory: &Path, root: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory.join("frames"))?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("samples.jsonl"))?;
        let mut config = OcrConfig::new(root.join(".deps"));
        config.detection_min_side = 64;
        let engine = OcrEngine::load(config).await?;
        Ok(Self {
            directory: directory.into(),
            file,
            runtime: UiaRuntime::start(Default::default(), OperationOptions::default()).await?,
            input: InputService::new()?,
            observer: Observer::new(engine.clone()),
            engine,
            previous: HashMap::new(),
            previous_images: HashMap::new(),
            image_bytes: 0,
            sequence: clipboard::sequence(),
            clipboard: None,
            index: 0,
            screen: super::screen::Screen::start()?,
            clipboard_reader: argusflow_windows::ClipboardReader::start()?,
            isolated: std::env::var_os("ARGUSFLOW_ISOLATED_RECORDING").is_some(),
        })
    }
    pub async fn sample(&mut self) -> Result<()> {
        let window = WindowLocator::default()
            .find_all()?
            .into_iter()
            .find(|w| w.identity().require_foreground().is_ok());
        let Some(window) = window else {
            return Ok(());
        };
        if self.isolated
            && !(window.title().starts_with("ArgusFlow Evidence")
                || (window.class_name() == "Notepad" && window.title().contains("Observed-")))
        {
            return Ok(());
        }
        // 此 demo 的授权应用范围，不包含条目、顺序或接收人。
        let chrome = window.class_name() == "Chrome_WidgetWin_1"
            && window.title().ends_with(" - Google Chrome");
        if !chrome && window.class_name() != "Notepad" && window.title() != "微信" {
            return Ok(());
        }
        if self.index >= 500 {
            return Err("达到500帧原型预算，停止录制".into());
        }
        if self.image_bytes >= 1024 * 1024 * 1024 {
            return Err("达到1GiB图像预算，停止录制".into());
        }
        let from = argusflow_windows::listening::qpc();
        let desktop = Desktop {
            input: self.input.clone(),
            window: window.identity(),
        };
        let result=async {
            let bounds=desktop.bounds()?;
            let frame=if window.title()=="微信"{self.screen.frame(&desktop).await?}else{Frame::from_bgrx(bounds,capture::capture(&desktop,bounds,bounds).await?)?};
            let frame_end=argusflow_windows::listening::qpc();
            let focus=self.runtime.observe_target(None,OperationOptions::default()).await.ok();
            if focus.as_ref().is_some_and(|f|f.target.password) {return Err::<Json,Box<dyn std::error::Error>>("密码控件，省略观察".into());}
            let mut texts=vec![];
            if window.class_name()=="Notepad" {
                let query=compile("document(text contains \"\")")?.bind(&Bindings::new())?;
                let found=self.runtime.query_aql(desktop.window.clone(),query,&Operation::new(OperationOptions::default())).await?;
                for item in found {if let Some(Value::Text(text))=item.attributes().get(&Attribute::Text) {texts.push(text.replace("\r\n","\n").replace('\r',"\n"));} item.handle().release();}
            }
            let mut ocr=vec![];
            let mut editor_empty=false;
            if window.title()=="微信" || self.isolated {
                let observation=self.observer.recognize_frame(&desktop,frame.clone()).await?;
                if window.title()=="微信" {editor_empty=super::editor_state::is_empty(&observation,super::wechat_support::layout::Zone::Editor.region(bounds)?);}
                for block in observation.blocks.iter() {ocr.push(json!({"text":block.text,"confidence":block.confidence,"rect":[block.rect.x(),block.rect.y(),block.rect.width(),block.rect.height()]}));}
            }
            let changed=self.previous.get(&desktop.window.handle()).filter(|p|p.bounds==frame.bounds).map(|p|p.pixels.as_chunks::<3>().0.iter().zip(frame.pixels.as_chunks::<3>().0).filter(|(a,b)|a!=b).count());
            let image=if changed==Some(0){self.previous_images.get(&desktop.window.handle()).ok_or("缺少前帧路径")?.clone()}else{
                let image=format!("frames/{:05}.bmp",self.index);
                save(&frame,&self.directory.join(&image))?;
                self.image_bytes+=frame.pixels.len()+frame.bounds.height() as usize*3+54;
                self.previous_images.insert(desktop.window.handle(),image.clone());
                image
            };
            self.previous.insert(desktop.window.handle(),frame);
            let clipboard_observation=self.clipboard_reader.observe(Some(self.sequence),OperationOptions::default()).await?;
            if clipboard_observation.sequence!=self.sequence {self.clipboard=match &clipboard_observation.content {argusflow_input_contracts::ClipboardContent::Text{text,..}=>Some(text.clone()), _=>None};self.sequence=clipboard_observation.sequence;}
            desktop.window.require_foreground()?;
            Ok(json!({"id":self.index,"from_qpc":from,"frame_through_qpc":frame_end,"through_qpc":argusflow_windows::listening::qpc(),"epoch_ms":epoch_ms(),"window":{"handle":desktop.window.handle(),"pid":desktop.window.process_id(),"class":window.class_name(),"title":window.title()},"bounds":[bounds.x(),bounds.y(),bounds.width(),bounds.height()],"image":image,"changed_pixels":changed,"focus":focus,"documents":texts,"clipboard_observation":clipboard_observation,"clipboard_sequence":self.sequence,"clipboard":self.clipboard,"ocr":ocr,"editor_empty":editor_empty}))
        }.await;
        match result {
            Ok(value) => write(&mut self.file, &value)?,
            Err(error) => write(
                &mut self.file,
                &json!({"id":self.index,"error":error.to_string(),"from_qpc":from,"through_qpc":argusflow_windows::listening::qpc()}),
            )?,
        }
        self.index += 1;
        Ok(())
    }
    pub async fn shutdown(self) -> Result<()> {
        self.file.sync_all()?;
        self.clipboard_reader
            .shutdown(OperationOptions::default())
            .await?;
        self.screen.shutdown().await?;
        self.input.shutdown(OperationOptions::default()).await?;
        self.runtime.shutdown(OperationOptions::default()).await?;
        self.engine.shutdown(OperationOptions::default()).await?;
        Ok(())
    }
}
fn save(frame: &Frame, path: &Path) -> Result<()> {
    let width = frame.bounds.width();
    let height = frame.bounds.height();
    let stride = (width as usize * 3 + 3) & !3;
    let mut bmp = vec![];
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((54 + stride * height as usize) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(width as i32).to_le_bytes());
    bmp.extend_from_slice(&(-(height as i32)).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&24u16.to_le_bytes());
    bmp.extend_from_slice(&[0; 24]);
    for row in frame.pixels.chunks_exact(width as usize * 3) {
        bmp.extend_from_slice(row);
        bmp.resize(bmp.len() + stride - row.len(), 0);
    }
    std::fs::write(path, bmp)?;
    Ok(())
}
