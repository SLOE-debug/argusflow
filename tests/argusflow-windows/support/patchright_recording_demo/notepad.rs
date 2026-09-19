//! 商店版记事本通过系统激活；窗口定位、读取、输入复用现有能力。
use super::Result;
use argusflow_aql::{Attribute, Bindings, Value, compile};
use argusflow_core::{Key, Operation, OperationOptions};
use argusflow_windows::{
    InputAction, InputService, Predicate, Query, SearchScope, UiaRuntime, WindowIdentity,
    WindowLocator,
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use windows::{
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    core::{PCWSTR, w},
};

/// 实机观察得到的定位器，回放时重新查询。
pub const EDITOR_QUERY: &str = "document(uia.class_name = \"RichEditD2DPT\", text contains \"\")";
pub struct Notepad {
    pub window: WindowIdentity,
    runtime: UiaRuntime,
    input: InputService,
    path: PathBuf,
    expected: String,
}
impl Notepad {
    pub fn input(&self) -> InputService {
        self.input.clone()
    }
    pub fn runtime(&self) -> &UiaRuntime {
        &self.runtime
    }
    pub async fn open(path: &Path) -> Result<Self> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        let argument: Vec<u16> = format!("\"{}\"", path.display())
            .encode_utf16()
            .chain([0])
            .collect();
        // 商店版复用已有进程，不能用拥有进程树的 Application 认领它。
        // SAFETY: 字符串 NUL 终止且调用期间存活，只打开本次新建文件。
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                w!("notepad.exe"),
                PCWSTR(argument.as_ptr()),
                None,
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err("Windows 记事本系统激活失败".into());
        }
        let filename = path.file_name().ok_or("缺少文件名")?.to_string_lossy();
        let window = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let mut found: Vec<_> = WindowLocator {
                    class_name: Some("Notepad".into()),
                    ..Default::default()
                }
                .find_all()?
                .into_iter()
                .filter(|w| w.title().starts_with(filename.as_ref()))
                .collect();
                match found.len() {
                    1 => return Ok::<_, Box<dyn std::error::Error>>(found.remove(0).identity()),
                    0 => tokio::time::sleep(Duration::from_millis(100)).await,
                    _ => return Err("本次记事本文件匹配多个窗口".into()),
                }
            }
        })
        .await??;
        let runtime = UiaRuntime::start(Default::default(), OperationOptions::default()).await?;
        // 标题更新和 WinUI 编辑控件创建并非原子操作；只读等待实际编辑区就绪。
        let ready = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match runtime
                    .find_unique(
                        Query {
                            window: window.clone(),
                            predicate: Predicate::ClassName("RichEditD2DPT".into()),
                            scope: SearchScope::Descendants,
                        },
                        OperationOptions::default(),
                    )
                    .await
                {
                    Ok(handle) => {
                        handle.release();
                        return Ok::<_, Box<dyn std::error::Error>>(());
                    }
                    Err(error) if error.kind() == argusflow_core::FailureKind::NotFound => {
                        tokio::time::sleep(Duration::from_millis(50)).await
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        })
        .await;
        if !matches!(ready, Ok(Ok(()))) {
            runtime.shutdown(OperationOptions::default()).await?;
            ready??;
        }
        Ok(Self {
            window,
            runtime,
            input: InputService::new()?,
            path: path.into(),
            expected: String::new(),
        })
    }
    async fn focus(&self) -> Result<()> {
        let operation = Operation::new(OperationOptions::default());
        super::taskbar::activate(
            &self.runtime,
            &self.input,
            &self.window,
            "Appid: Microsoft.WindowsNotepad_8wekyb3d8bbwe!App",
        )
        .await?;
        let current = WindowLocator {
            process_id: Some(self.window.process_id()),
            class_name: Some("Notepad".into()),
            ..Default::default()
        }
        .find_all()?;
        let name = self.path.file_name().ok_or("缺少文件名")?.to_string_lossy();
        if !current.iter().any(|w| {
            w.identity().handle() == self.window.handle()
                && w.title().trim_start_matches('*').starts_with(name.as_ref())
        }) {
            return Err("记事本已切换至其他文档，停止输入".into());
        }
        let editor = self
            .runtime
            .find_unique(
                Query {
                    window: self.window.clone(),
                    predicate: Predicate::ClassName("RichEditD2DPT".into()),
                    scope: SearchScope::Descendants,
                },
                OperationOptions::default(),
            )
            .await
            .map_err(|error| format!("定位记事本编辑区：{error}"))?;
        let result = self.runtime.focus_aql(&editor, &operation).await;
        editor.release();
        result?;
        if let Err(error) = self.window.require_foreground() {
            for window in WindowLocator::default().find_all()? {
                if window.identity().require_foreground().is_ok() {
                    eprintln!("实际前台窗口：{window:?}");
                }
            }
            return Err(error.into());
        }
        Ok(())
    }
    async fn keys(&self, keys: Vec<Key>) -> Result<()> {
        self.input
            .perform(
                self.window.clone(),
                InputAction::Chord(keys),
                OperationOptions::default(),
            )
            .await?;
        Ok(())
    }
    pub async fn read_text(&self, query: &str) -> Result<String> {
        self.window.validate()?;
        let matches = self
            .runtime
            .query_aql(
                self.window.clone(),
                compile(query)?.bind(&Bindings::new())?,
                &Operation::new(OperationOptions::default()),
            )
            .await?;
        let result = match matches.as_slice() {
            [item] => match item.attributes().get(&Attribute::Text) {
                Some(Value::Text(text)) => Ok(text.replace("\r\n", "\n").replace('\r', "\n")),
                _ => Err("记事本未暴露 TextPattern 文本".into()),
            },
            _ => Err("记事本编辑区不唯一".into()),
        };
        for item in matches {
            item.handle().release();
        }
        result
    }
    pub async fn append(&mut self, text: &str, query: &str) -> Result<()> {
        if self.read_text(query).await?.trim_end_matches('\n')
            != self.expected.trim_end_matches('\n')
        {
            return Err("记事本内容已被外部修改".into());
        }
        self.keys(vec![Key::Control, Key::End]).await?;
        self.input
            .perform(
                self.window.clone(),
                InputAction::Text(text.into()),
                OperationOptions::default(),
            )
            .await?;
        self.keys(vec![Key::Enter]).await?;
        self.expected.push_str(text);
        self.expected.push('\n');
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if self.read_text(query).await?.trim_end_matches('\n')
                    == self.expected.trim_end_matches('\n')
                {
                    return Ok::<_, Box<dyn std::error::Error>>(());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await??;
        Ok(())
    }
    pub async fn save(&self) -> Result<()> {
        self.focus().await?;
        self.keys(vec![Key::Control, Key::Letter('S')]).await?;
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let actual = std::fs::read_to_string(&self.path)?
                    .trim_start_matches('\u{feff}')
                    .replace("\r\n", "\n");
                if actual == self.expected {
                    return Ok::<_, Box<dyn std::error::Error>>(());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await??;
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<()> {
        // 不关闭共享的记事本进程，保留本次结果供查看。
        let input = self.input.shutdown(OperationOptions::default()).await;
        let uia = self.runtime.shutdown(OperationOptions::default()).await;
        input?;
        uia?;
        Ok(())
    }
}
