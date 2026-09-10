# AQL 首期

AQL 用于在显式来源中定位元素并执行一次点击或文字插入。`experimental` 只作为角色、属性与实现思路参考。英文是保存与执行契约，中文关键字只存在于前端语言服务。

## 编辑器

```text
窗口(名称 包含 "设置") >> 按钮(名称 = "保存", 可用 = 真)
```

对应英文文件：

```aql
window(name contains "设置") >> button(name = "保存", enabled = true)
```

安装与启动（Node 22、pnpm 10、Rust、`wasm32-unknown-unknown`）：

```powershell
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127 --locked
pnpm install --frozen-lockfile
pnpm dev
```

`pnpm dev/build/test` 自动编译 WASM 并生成绑定。生成目录不提交。编辑页提供中文编辑、英文预览、中文诊断、词汇补全、悬浮说明、格式化及 UTF-8 `.aql` 导入导出。页面仅使用本地 WASM，不连接桌面执行服务。Monaco 按需加载；组合输入期间暂停补全和格式化，英文预览与导出立即撤销。语法不完整的内容保留在内存草稿中；旧分析和迟到文件读取不能覆盖新输入。刷新页面不会持久化草稿。

转换基于保留空白与注释的词法标记；字符串、正则、CSS 字符串、注释和 `$参数名` 原样保留。双向 UTF-8 区间映射转换成 Monaco 的零基行与 UTF-16 列，支持中文、emoji 和 CRLF。完整词汇表位于 `crates/argusflow-aql-wasm/src/localization/dictionary.rs`。

输入关键字时自动显示候选，也可按 `Ctrl+Space` 主动请求。支持中文或英文前缀：`按`、`but` 都能找到“按钮”，`cla` 可以找到 `uia.class_name` 和 `dom.class` 对应的中文属性；选中候选后按 Enter 或 Tab 插入。角色和函数把光标放进括号，已有括号时只替换名称。输入法确认后恢复候选，组词期间不打断输入。`$` 前缀可引用当前查询中已有的参数名。

鼠标悬停在角色、函数、属性、运算符、布尔值或 `$参数` 上，会显示类别、类型或函数用法、中文说明和示例。有效查询中的参数类型由英文编译器确认；无效草稿标明“类型待确定”。字符串、正则和注释中的相同文字不会显示语言符号说明。

## 语法与类型

| 用途 | 英文 | 中文 |
| --- | --- | --- |
| 任意元素 | `element()` | `元素()` |
| 精确匹配 | `button(name = "保存")` | `按钮(名称 = "保存")` |
| 包含、前后缀 | `contains`、`starts_with`、`ends_with` | 包含、开头为、结尾为 |
| 正则 | `name matches /保存.*/i` | `名称 匹配 /保存.*/i` |
| 逻辑 | `not`、`and`/`,`、`or` | 非、且/`,`、或 |
| 布尔 | `true`、`false` | 真、假 |
| 直接子级、后代 | `>`、`>>` | 原样 |
| 显式排序选择 | `first(button())`、`nth(button(), 2)` | `首个(按钮())`、`第几个(按钮(), 2)` |
| 原生 CSS | `css("#save")` | `CSS("#save")` |
| 显式文档边界 | `frame(query)`、`shadow(query)` | 框架、影子根 |

条件优先级为 `not > and/逗号 > or`，可用括号分组。角色括号可以为空。字符串使用 JSON 双引号转义；正则采用 Rust `regex` 语义，只支持可选 `i` 标志，不支持回溯引用或环视。包含、前后缀和精确匹配区分大小写。`nth` 从 1 开始，超出匹配范围得到空结果。`//` 和 `/* ... */` 注释参与格式化保留。

角色包括 window、dialog、pane、button、textbox、checkbox、radio、combobox、list、list_item、tree、tree_item、tab、tab_item、menu、menu_item、link、image、table、row、cell、document、text，以及任意角色的 element。

| 属性 | 类型 | 来源 |
| --- | --- | --- |
| `name`、`text` | 文本 | UIA、DOM、OCR |
| `key`、`value` | 文本 | UIA、DOM |
| `enabled`、`visible`、`focused`、`checked`、`selected` | 布尔 | UIA、DOM |
| `confidence` | 数值 | OCR |
| `uia.automation_id`、`uia.class_name`、`uia.accelerator_key`、`uia.access_key`、`uia.framework_id` | 文本 | UIA |
| `dom.id`、`dom.test_id`、`dom.class`、`dom.tag` | 文本 | DOM |

全部类型支持 `=`、`!=`；文本另支持包含、前后缀和正则，数值另支持 `< <= > >=`。参数类型从比较属性推导，缺失、多余或类型错误均拒绝绑定。正则和 CSS 要求源码字面量，参数用于普通比较值。未知属性保持缺失；即使 `not value = ""` 也不会把没有 value 的元素误判为匹配。UIA 的 `key` 对应 AutomationId，DOM 对应 id，OCR 不支持 key。

## Rust 接口

```rust
use argusflow_aql::{Bindings, Value, compile};
use argusflow_automation::{Locator, QuerySource};
use argusflow_core::OperationOptions;

// page 是调用者已经附加的 Page。
let query = compile("button(name = $名称, enabled = true)")?;
let bindings = Bindings::from([("名称".into(), Value::Text("保存".into()))]);
let locator = Locator::bind(QuerySource::Browser(page), &query, &bindings)?;
let matches = locator.find_all(OperationOptions::default()).await?;
let unique = locator.find_unique(OperationOptions::default()).await?;
locator.click(OperationOptions::default()).await?;
```

UIA 来源为 `QuerySource::Uia { runtime, window, input }`；OCR 为 `QuerySource::Ocr { sampler, source, region, window, input }`。OCR 的 region 是指定采样来源的本地像素矩形，window 是输入时验证的窗口身份。`LocatedElement` 分别持有 UIA 租约、浏览器 frame/文档身份，或 OCR 文字和采样版本。UIA/OCR 装配在 Windows 可用；跨平台仍可独立使用语言、浏览器和 OCR 图片查询能力。

```rust
let query = compile("textbox(name = $名称)")?;
let locator = Locator::bind(source, &query, &Bindings::from([
    ("名称".into(), Value::Text("备注".into())),
]))?;
locator.type_text("追加内容", OperationOptions::default()).await?;
```

每次 `click/type_text` 都重新查询并验证目标。默认零结果返回 `NotFound`，多结果返回 `Ambiguous`；只有查询中显式 `first/nth` 才选择某个匹配。UIA/DOM 使用树先序，OCR 使用识别结果的阅读顺序。超过预算直接报错，不返回截断成功。

各方法还提供 `*_with_operation(&Operation)`，用于外部取消或沿用更早开始的总截止时间。定位、聚焦、输入共用同一票据。已有 `InputService` 与 CDP 输入额度在聚焦到输入完成期间保持独占；并发冲突返回 `Busy`。错误保留请求、阶段与 `Effect`；超时或取消后不自动重放动作。

UIA 在 MTA 内读取属性、遍历关系、管理租约，并使用 Per Monitor V2 物理坐标。点击前复验窗口归属和 UIA 命中点；UIA/OCR 真实输入要求目标窗口已在前台且点击点未被其他窗口遮挡，不自动激活。UIA 文字输入要求可见、启用、可写的 Edit 控件及 Value/Text Pattern，聚焦后复验实际焦点并把选区收起到末尾。

DOM 输入支持具有可控光标的 text/search/password/url/tel、textarea 与 contenteditable；已有选区收起到末尾再通过 CDP 插入。只读、禁用或不提供安全选区接口的控件明确报 `Unsupported`。OCR 只识别 text/element、name/text/confidence，不推断按钮或可编辑性；输入前精确复验识别区域版本，单击文字位置建立焦点，再发送文字。OCR 不能判断点击位置是否为输入控件，调用方应绑定实际输入区域。

## 浏览器边界

```aql
frame(css("iframe[name=payment]")) >> textbox(name = "卡号")
shadow(css("settings-panel")) >> button(name = "保存")
frame(css("iframe")) >> shadow(css("custom-form")) >> textbox()
```

`frame/shadow` 的宿主必须唯一；普通 `>`、`>>` 不隐式进入 iframe 或 Shadow Root。边界自身不能作为结果，必须继续指定内部目标。支持同进程、跨域 OOPIF 及嵌套边界；仅支持开放 Shadow Root。跨进程 iframe 按 frame 身份附加独立平坦 CDP 子会话，附加后核对 frame 树，持有结果期间保留会话租约，释放后只脱离自有会话。

名称与角色来自 CDP Accessibility；DOM 当前属性和 CSS 成员通过固定脚本读取，AQL 条件与正则统一在 Rust 求值。文档导航、上下文销毁、节点/属性变化和会话撤销使身份失效。动作开始后如果页面变化导致失败，不重新定位重放。

点击点逐级从子 frame 视口投影到顶层视口，包含边框、缩放和负坐标，并逐层验证命中。首期拒绝带旋转、倾斜或透视的 iframe，避免使用未经验证的位置。

## 资源与边界

源码最多 64 KiB，语法嵌套最多 64 层、表达式最多 256 项；正则编译预算 1 MiB。浏览器 DOM/AX 节点最多 10000，树深度与文档边界最多 64，结果数复用连接配置（默认 256），协议消息复用 8 MiB 上限。UIA 复用现有 UiaConfig 预算；OCR 复用 3000 文本块上限及采样缓存，查询最多返回 256 项。文字输入最多 16384 UTF-8 字节。

本期没有聚合、分组、关联、空间关系、自动后端选择、动作脚本、工作流或桌面执行服务。构建、原生验收、默认跳过项和限制见 [AQL 验证记录](aql-validation.md)。可编译的手动浏览器示例位于 `tests/argusflow-automation/support/browser.rs`，运行前需调用者提供调试端点，本次开发未执行真实浏览器验收。
