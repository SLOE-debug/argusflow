//! 单目录原子文件存储；内容版本冲突不会覆盖用户外部修改。
use super::{DocumentSummary, LoadedDocument, WorkflowFile, wire::decode_workflow};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// 只操作已打开目录内的当前格式文档。
pub struct Workspace {
    root: PathBuf,
}
impl Workspace {
    /// 创建持久目录后打开；重复调用不会清空已有工作流。
    pub fn initialize(path: &Path) -> Result<Self, String> {
        if !path.is_dir() {
            fs::create_dir_all(path)
                .map_err(|error| format!("无法创建工作流数据目录 {}：{error}", path.display()))?;
        }
        let workspace = Self::open(path)?;
        // 尽早报告不可写目录，避免欢迎页允许创建却无法保存。
        let probe = tempfile::NamedTempFile::new_in(&workspace.root)
            .map_err(|error| format!("无法写入工作流数据目录 {}：{error}", path.display()))?;
        probe
            .close()
            .map_err(|error| format!("无法清理目录写入检查文件：{error}"))?;
        Ok(workspace)
    }
    /// 打开真实目录，路径由应用装配或测试传入。
    pub fn open(path: &Path) -> Result<Self, String> {
        let root = path
            .canonicalize()
            .map_err(|error| format!("无法访问工作流数据目录 {}：{error}", path.display()))?;
        if !root.is_dir() {
            return Err("工作流数据路径不是目录".into());
        }
        Ok(Self { root })
    }
    /// 规范化后的显示路径。
    pub fn path(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }
    /// 列出目录内通过当前结构校验的文档，读取失败直接报告。
    pub fn list(&self) -> Result<Vec<DocumentSummary>, String> {
        let mut documents = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".workflow.json"))
            {
                continue;
            }
            let loaded = read(&path)?;
            if path.file_name().and_then(|name| name.to_str())
                != Some(&format!("{}.workflow.json", loaded.file.id))
            {
                return Err(format!("文件名与工作流身份不一致：{}", path.display()));
            }
            documents.push(DocumentSummary {
                id: loaded.file.id,
                name: loaded.file.definition["name"]
                    .as_str()
                    .unwrap_or("未命名")
                    .into(),
                revision: loaded.revision,
            });
        }
        documents.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(documents)
    }
    /// 加载文档及当前磁盘版本。
    pub fn load(&self, id: &str) -> Result<LoadedDocument, String> {
        read(&self.file_path(id)?)
    }
    /// 原子保存；None 只允许首次创建，版本必须与磁盘相同。
    pub fn save(
        &self,
        file: &WorkflowFile,
        revision: Option<&str>,
    ) -> Result<LoadedDocument, String> {
        validate(file)?;
        let path = self.file_path(&file.id)?;
        match (path.exists(), revision) {
            (true, Some(expected)) if read(&path)?.revision == expected => {}
            (false, None) => {}
            _ => return Err("conflict:文件已在外部修改，请重新载入或另存副本".into()),
        }
        let bytes = serde_json::to_vec_pretty(file).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("编辑文件超过 16 MiB".into());
        }
        let mut temp = tempfile::NamedTempFile::new_in(&self.root).map_err(|e| e.to_string())?;
        temp.write_all(&bytes).map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        // 在提交前再次检查，缩小外部写入与原子替换之间的竞态窗口。
        if let Some(expected) = revision
            && read(&path)?.revision != expected
        {
            return Err("conflict:保存期间文件已变化".into());
        }
        if revision.is_some() {
            temp.persist(&path).map_err(|e| e.to_string())?;
        } else {
            temp.persist_noclobber(&path)
                .map_err(|e| format!("conflict:首次创建文件失败：{e}"))?;
        }
        Ok(LoadedDocument {
            file: file.clone(),
            revision: blake3::hash(&bytes).to_hex().to_string(),
        })
    }
    /// 删除指定版本的文件，外部修改过的文档必须重新确认。
    pub fn remove(&self, id: &str, revision: &str) -> Result<(), String> {
        let path = self.file_path(id)?;
        if read(&path)?.revision != revision {
            return Err("conflict:工作流已在外部修改，请刷新列表后重试删除".into());
        }
        fs::remove_file(path).map_err(|error| format!("删除工作流失败：{error}"))
    }
    fn file_path(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty()
            || id.len() > 100
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err("工作流身份只能包含字母、数字、下划线和短横线".into());
        }
        Ok(self.root.join(format!("{id}.workflow.json")))
    }
}
fn read(path: &Path) -> Result<LoadedDocument, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return Err("文件类型或大小无效".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let file: WorkflowFile =
        serde_json::from_slice(&bytes).map_err(|error| format!("{}：{error}", path.display()))?;
    validate(&file)?;
    Ok(LoadedDocument {
        file,
        revision: blake3::hash(&bytes).to_hex().to_string(),
    })
}
/// 保存允许未完成的业务配置，但不接受损坏结构和非有限布局。
pub fn validate(file: &WorkflowFile) -> Result<(), String> {
    validate_draft(file, true)
}
/// 剪贴板局部选区不要求起止布局，其他当前格式结构约束完全相同。
pub(super) fn validate_draft(file: &WorkflowFile, require_endpoints: bool) -> Result<(), String> {
    let workflow = decode_workflow(&file.definition)?;
    let nodes = super::structure::validate(&workflow)?;
    let mut edges = std::collections::BTreeSet::new();
    for scope in &workflow.scopes {
        argusflow_workflow::ScopeGraph::new(scope)
            .map_err(|error| format!("{}：{}", scope.id, error.message))?;
        for edge in &scope.edges {
            if !edges.insert(edge.id.as_str()) || !file.editor.edges.contains_key(&edge.id) {
                return Err(format!("{}：连线身份重复或缺少端口布局", scope.id));
            }
            if !require_endpoints {
                for endpoint in [&edge.source, &edge.target] {
                    let kind = match endpoint {
                        argusflow_workflow::EdgeEndpoint::Start => "start",
                        argusflow_workflow::EdgeEndpoint::End => "end",
                        argusflow_workflow::EdgeEndpoint::Node { .. } => continue,
                    };
                    if !file
                        .editor
                        .nodes
                        .contains_key(&format!("${kind}:{}", scope.id))
                    {
                        return Err(format!("{}：选区连线引用了未复制的起止卡片", scope.id));
                    }
                }
            }
        }
        if require_endpoints {
            for (kind, label) in [("start", "开始"), ("end", "结束")] {
                if !file
                    .editor
                    .nodes
                    .contains_key(&format!("${kind}:{}", scope.id))
                {
                    return Err(format!("{}：缺少{label}，无法保存", scope.id));
                }
            }
        }
    }
    if file
        .editor
        .edges
        .keys()
        .any(|id| !edges.contains(id.as_str()))
    {
        return Err("存在无对应连线的端口布局".into());
    }
    if nodes.iter().any(|id| !file.editor.nodes.contains_key(*id)) {
        return Err("节点缺少画布布局".into());
    }
    if file.editor.nodes.values().any(|node| {
        !node.x.is_finite() || !node.y.is_finite() || node.x.abs() > 1e9 || node.y.abs() > 1e9
    }) {
        return Err("节点坐标无效".into());
    }
    Ok(())
}
