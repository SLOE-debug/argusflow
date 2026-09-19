//! demo 专用强类型 workflow；由实际成功动作生成，独立于产品编辑器。
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    BrowserOpen { url: String },
    NotepadOpen,
    BrowserRead { selector: String, index: usize },
    NotepadAppend { query: String, prefix_run_id: bool },
    NotepadSave,
    NotepadRead { query: String, line: usize },
    WechatSend { recipient: String },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    pub format: String,
    pub steps: Vec<Step>,
}
impl Workflow {
    /// 录制操作者要执行的动作；成功的动作逐条提交为录制结果。
    pub fn demonstration() -> Self {
        let mut steps = vec![
            Step::BrowserOpen {
                url: "https://www.baidu.com/".into(),
            },
            Step::NotepadOpen,
        ];
        for index in [0, 3, 4] {
            steps.extend([
                Step::BrowserRead {
                    selector: "#hotsearch-content-wrapper li a".into(),
                    index,
                },
                Step::NotepadAppend {
                    query: super::notepad::EDITOR_QUERY.into(),
                    prefix_run_id: true,
                },
                Step::NotepadSave,
            ]);
        }
        for line in 0..3 {
            steps.extend([
                Step::NotepadRead {
                    query: super::notepad::EDITOR_QUERY.into(),
                    line,
                },
                Step::WechatSend {
                    recipient: "文件传输助手".into(),
                },
            ]);
        }
        Self {
            format: "argusflow-patchright-demo-v1".into(),
            steps,
        }
    }
    pub fn validate(&self) -> super::Result<()> {
        if self.format != "argusflow-patchright-demo-v1"
            || self.steps.is_empty()
            || self.steps.len() > 100
        {
            return Err("不支持的 demo workflow 格式或步骤数量".into());
        }
        for step in &self.steps {
            match step {
                Step::BrowserOpen { url } if url != "https://www.baidu.com/" => {
                    return Err("demo 仅访问百度首页".into());
                }
                Step::WechatSend { recipient } if recipient != "文件传输助手" => {
                    return Err("demo 仅发送至文件传输助手".into());
                }
                _ => {}
            }
        }
        Ok(())
    }
}
