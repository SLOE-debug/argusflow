# Workflow 自动化任务目录

所有任务 `version = 1`。`config` 为静态 JSON，`inputs` 为具名 Expr，`resources` 为端口到可见资源名称，`resource_outputs` 为端口到当前作用域新名称。除明确列出的字段外，静态配置拒绝未知字段。

资源类型：`automation.browser`、`automation.page`、`automation.query_source`、Windows 的 `automation.application` 与 `automation.window`。

| type_id | config | 数据输入 | 借用资源 → 创建资源 | 数据输出 |
| --- | --- | --- | --- | --- |
| source.host | `{name}`，必须是宿主注册名称 | 无 | 无 → source | 无 |
| browser.launch | `{headless?: bool}` | executable: text | 无 → browser | 无 |
| browser.connect | `{}` | endpoint: text | 无 → browser | 无 |
| browser.pages | `{}` | 无 | browser → 无 | pages: list(record{id,title,url: text}) |
| browser.attach | `{}` | target_id: text | browser → page | 无 |
| browser.new_page | `{}` | url: text | browser → page | 无 |
| browser.navigate | `{}` | url: text | page → 无 | 无 |
| source.dom | `{}` | 无 | page → source | 无 |
| aql.query | `{query: text}` | 全部 AQL 参数 | source → 无 | matches: list(Match) |
| aql.exists | `{query: text}` | 全部 AQL 参数 | source → 无 | exists: bool |
| aql.wait | `{query, interval_ms, present}` | 全部 AQL 参数 | source → 无 | exists: bool |
| aql.click | `{query: text}` | 全部 AQL 参数 | source → 无 | 无 |
| aql.type_text | `{query: text}` | 全部 AQL 参数及 text: text | source → 无 | 无 |
| application.launch | `{visible: bool}` | executable: text, arguments: list(text) | 无 → application | 无 |
| application.wait_window | `{title?: text, class_name?: text}` | 无 | application → window | 无 |
| window.attach | `{title?: text, class_name?: text}` | process_id: int | 无 → window | 无 |
| window.activate | `{}` | 无 | window → 无 | 无 |
| source.uia | `{}` | 无 | window → source | 无 |

只有 `browser.pages`、`aql.query`、`aql.exists` 和 `aql.wait` 声明 `safe_to_retry`。`aql.wait` 的间隔为 10..=60000 毫秒，轮询到存在性与 present 相符；节点时限覆盖整个等待。底层查询错误直接传播，不把服务失败当作不存在。

AQL 参数类型来自英文查询编译器，文字→Text、布尔→Bool、Number→Float。`aql.type_text` 的 `text` 是保留输入端口，查询参数不得同名。点击和输入每次重新定位，遵循 Locator 的唯一性和内容复验，不复用快照中的旧句柄。

Match 为固定记录：`source: text`（dom/uia/ocr）、`space: text`（frame_css_pixels/screen_physical_pixels）、`frame: optional(text)`、`name: optional(text)`、`text: optional(text)`、`confidence: optional(float)`、`bounds: list(float)`。bounds 的四项统一是 left、top、right、bottom；DOM 带 frame ID 且保留该 frame 的 CSS 像素空间，UIA/OCR 为屏幕物理像素。不存在的属性保持 Optional(None)，不填造值。快照不返回平台操作句柄。

资源的关闭使用内置 `Action::Release` 或作用域自动回收，没有隐式跨级关闭节点。Windows 节点仅在 Windows 宿主注册；缺失能力在准备阶段明确报错。
