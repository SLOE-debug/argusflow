# Semantic Recorder 第一阶段

`argusflow-recorder` 实现 Physical → Semantic。它通过 `argusflow-core` 的 `WindowInspector` / `TargetInspector` 只读契约使用已有的 `CdpRuntime`、`UiaRuntime`、`VisionRuntime`。`ActionRouter` 继续负责 Semantic → Physical，没有注册录制逻辑，也没有复制动作执行栈。

## 入口与输出

Tauri 已装配以下命令：

| 命令 | 行为 |
| --- | --- |
| `start_recording` | 显式安装 Windows 全局低级鼠标、键盘 Hook；返回录制 ID 和状态。重复调用报错。 |
| `get_recording_status` | 返回 phase、recording_id、started_at_unix_ms、processed_events、dropped_events。 |
| `stop_recording` | 卸载 Hook，排空 worker，规范化并保存记录；返回 `CompletedRecording { files, trace }`。保存失败可再次调用重试。 |
| `list_recordings` | 返回最近最多 100 次完整保存的录制摘要，不包含输入文字。 |
| `get_recording` | 按 UUID 读取已脱敏的完整录制，拒绝任意路径。 |

首页和编辑器的标题栏均提供“录制操作”入口。面板可开始录制、停止并保存、显示计时与已处理/丢弃事件数、查看历史、浏览操作及其语义实体/定位候选/降级诊断，并分别查看、复制或下载完整的 Raw/Semantic JSON。列表与 JSON 按每页 50 条显示，复制和导出包含完整层；自动保存的绝对文件路径也在面板中提供。

界面控制器位于 `src/features/recorder`，只处理类型、IPC 和生命周期；视觉组件位于 `src/components/recorder` 并复用通用 Button/Dialog。收起面板继续录制，标题栏保留录制状态。页面恢复会读取现有后台状态，不自动安装 Hook，也不因组件卸载而擅自停止。普通浏览器预览明确禁用录制入口。工作流运行期间不能从面板开始录制，录制期间禁用界面的工作流运行入口。

生命周期明确分成 `idle`、`recording`、`finishing`、`awaiting_save`。保存失败后 Hook 已卸载，面板显示“重试保存”，不会误报仍在监听。构造 AppState 不会开始监听输入，只有 `start_recording` 安装 Hook；使用后应调用 `stop_recording` 完成持久化。桌面正常退出也会先停止并保存录制，再关闭 capture 服务；进程强制终止不保证保存。

默认目录为启动工作目录下的 `.argusflow/recordings/<UUID>/`，已被仓库现有 `.gitignore` 覆盖：

- `raw.json`：`RawTrace`，包括原始事件序号、Win32 timestamp、展开回绕后的相对时间、已脱敏输入、解析快照和输入诊断。
- `semantic.json`：`NormalizedSemanticTrace`，可单独发送给 AI。每条记录包括操作、时间、原始事件编号和已脱敏原始输入，以及 application/window context、backend、entity、selector candidates、preferred selector 索引、confidence 和 fallback diagnostics。
- `manifest.json`：协议版本、录制 ID、开始时间和丢弃计数；最后发布，表示两份 trace 都已写完。

文件先写 `.pending` 再 rename 发布。`RecordingFiles` 返回绝对路径。Raw Trace 与 Semantic Trace 都经过脱敏，Raw 不代表允许落盘密码键码。

## 输入与并发

专用 Hook 消息线程只读取 `MSLLHOOKSTRUCT` / `KBDLLHOOKSTRUCT` 的 timestamp、point、button、vk、scan code、flags，并 `try_send` 到容量 4096 的队列；回调始终调用 `CallNextHookEx`。忽略 injected input。回调没有窗口发现、COM、UIA、CDP、OCR、Unicode 转换、日志或文件写入。

摄入线程先冻结廉价 Win32 上下文和目标线程键盘布局，再投递容量 2048 的异步队列。语义 worker 最多同时检查 16 个事件，按捕获顺序释放结果，最后统一规范化。Raw Trace 上限 100,000 个事件；溢出保留计数与 sequence 缺口，禁止跨缺口合并文本或鼠标事件。

晚于事件 150ms 才开始窗口采样，或异步排队超过 150ms 的事件不会用当前 UI 伪装历史目标。键盘 pending 检查若跨越可能改变焦点的点击/组合键代数，会降级并遮盖输入。窗口销毁、复用或移动也产生显式诊断。

异步观察无法冻结第三方应用在按下后的 UI。立即消失的菜单、程序主动变更焦点等仍存在竞态；trace 记录观察结果与可检测的降级，不能宣称所有输入都有无竞态的语义快照。

## Target Resolution

1. Mouse down 使用 `WindowFromPoint(point)` → `GetAncestor(GA_ROOT)` → HWND/PID/EXE。`GetForegroundWindow` 只用于键盘 context 和 CDP 活动页面交叉校验，不替代点击窗口定位。
2. Managed CDP 要求 PID/EXE 属于当前 runtime 已启动且仍 attach 的 BrowserSession。当前原生键盘窗口与该 document 都必须拥有焦点，以避免同进程其他窗口/标签的错误映射。不发现任意 Chrome，不创建新 attach。
3. 原生 `Chrome_RenderWidgetHostHWND` 的唯一可见 client rect 提供物理 viewport 原点。验证 native width/height 与 CSS innerWidth/innerHeight × devicePixelRatio 一致后，使用 `(screenPhysical - viewportPhysicalOrigin) / DPR` 调用 `DOM.getNodeForLocation`。DPR 包括浏览器 page zoom；屏幕原点不乘 DPI，不加 document scroll offset。负屏幕坐标受支持。
4. `DOM.resolveNode` + 固定只读页面函数采集与现有 AQL matcher 一致的 DOM role/name/data-testid/id/class、祖先、CSS bounds 与敏感性，再转换为屏幕物理 bounds。每次检查独立 object group，正常返回和取消均释放。没有读取 `.value`、outerHTML 或任意 HTML 属性集合。
5. CDP 不适用/失败时，在现有 UIA MTA worker 上使用 `ElementFromPointBuildCache`；键盘使用 `GetFocusedElementBuildCache`。同步反查期间设置 Per-Monitor V2 线程 DPI context，并在结束时恢复，避免 Win32 UIA proxy 将点击点与 cache bounds 虚拟化到错误控件。CacheRequest 采集 ControlType、Name、AutomationId、ClassName、FrameworkId、bounds、IsPassword、PID、native handle、offscreen/focus。沿有限祖先链验证元素属于实际根 HWND；不读取 ValuePattern。
6. UIA 无有效结果时复用 `VisionRuntime.current_scene` 的窗口 capture + OCR，按 Scene 实际 screen origin 将点转换成帧本地物理坐标，选择包含该点的最小有效 OCR 文本框。不会把最近但不包含该点的文本硬当成目标。
7. 最后保留 coordinate 候选、窗口上下文与失败链。键盘无可靠焦点实体时仅保留窗口上下文，绝不猜坐标。

浏览器 toolbar、后台页面、缺少/多个 renderer、无法确认几何、pinch zoom/device emulation，以及当前执行器无法精确查询的 iframe/shadow scope 会回退。普通浏览器 page zoom 与显示器 DPI 已支持。Vision 无法读取键盘焦点，因此不用于推测文本输入字段。

## Selector Synthesizer

候选由已观察事实构造 AQL AST，再交给现有 canonical formatter 生成 AQL v3，不拼接未转义 AQL、不调用 AI。AutomationId/data-testid 优先，其后是稳定 DOM id、稳定祖先关系和 role+name；class、明显生成的 hash/id 与坐标降权。没有观察到结果顺序就不生成 `Nth`。

`RecordedSelector` 明确区分 `Aql(AqlQuery)` 和 `Coordinate(ScreenPoint)`。`stability_score` 是确定性的 0–100 启发式，`confidence` 是 0–1 观察置信度。两者都不代表唯一性证明或回放成功率；trace 显式包含 `selector_uniqueness_unverified`，现有执行器仍负责查询歧义校验。

## Normalization 与脱敏

- 同一鼠标键的 down/up 合成 Click，保留 down 时解析的目标。超出 DPI 调整容差的移动（即使最终回到原点）视为不支持的拖拽；缺失配对不编造 Click。
- 同一语义实体上、间隔不超过 1 秒的连续字符归并为增量 `TypeText`。没有读取整个字段值，因此不生成冒充完整替换值的 SetValue。
- Ctrl+C 等组合键、Enter、Tab、Escape、Backspace/Delete、方向键、Home/End/PageUp/PageDown 生成现有 `KeyChord` 形式的 PressKey。同步扩展核心、Windows 回放与编辑器契约，编辑键簇回放带 E0 标记。
- 鼠标操作、PressKey、目标切换、超时、输入缺口和不支持的操作切断文本组。Raw 与 Semantic 之间保留事件引用以及对应的已脱敏输入。
- UIA IsPassword、DOM password/autocomplete/显式敏感标记和字段元数据识别 password/token/secret/验证码/银行卡等；敏感 entity name 和祖先聚合 name 不进入 selectors。
- 字段敏感性为 Sensitive 或 Unknown，或不能证明是可编辑字段时，字符变成 `Redacted`，down 和对应 up 的 vk/scan code 都为空；自动重复不能解除同一次按压的遮盖。安全的语义组合键仍可保留。不会读取剪贴板；Ctrl+V 保留为 PressKey，不编造粘贴文本。
- 使用目标线程布局及无副作用 `ToUnicodeEx(flags=4)`。IME composition/commit 和死键组合不由低级 Hook 可靠提供，本阶段保留不支持诊断，不把拼音/死键序列假装成提交文本。`keyboard_decode` 明确区分 `input_method_active`、`dead_key`、`missing_window`、`missing_thread`、`unsupported_chord`、`invalid_key` 和 `no_character`；即使没有产生语义步骤，失败原因仍进入独立 Semantic Trace 并在界面显示。Redaction 标记不含原文，Raw 仍保留输入事件时序和数量。

右键/中键 Click 会保留真实 button；现有 Workflow Click 只直接支持左键，后续工作流整理不能悄悄丢弃该差异。

## 后续 AI 边界

AI 输入为 `semantic.json`。允许删除噪声、合并步骤、抽象变量/输入、推断 wait/condition/loop，并使用已提供的 AQL v3 候选构造现有 `WorkflowDefinition`；不能把低置信度 coordinate 记录重新解释为未经观察的 UI，也不应把 redaction 标记当成要输入的字面文本。

本阶段不接 AI API、不自动生成 Condition/Loop、不做任意 Chrome attach、拖拽或高级滚轮语义。

## 验证

常规 Rust 单元/协议测试覆盖解析顺序、fallback、u32 时序相关队列契约、并发结果顺序、焦点代数/延迟脱敏、密码 down/up、编辑键、AQL 转义/稳定性、DPI/缩放/负坐标、分离存储和 CDP 对象释放。CDP 测试使用本地模拟 WebSocket，页面函数测试使用 Node 纯对象 fixture；这些常规测试不自动操作浏览器或安装真实全局 Hook。

独立 `#[ignore]` 桌面测试只在显式命令下运行：

- `windows_global_hooks_receive_filter_stop_and_restart`：真实安装 WH_MOUSE_LL / WH_KEYBOARD_LL，用带测试标记的往返一像素移动及 F24 down/up 验证 Windows 实际回调、注入过滤、卸载和三次重启。测试专用观察器只累计数量，发布构建不存在此观察器，也没有允许 injected input 进入生产录制的开关。
- `windows_global_hooks_capture_physical_input`：安装后最多等待 60 秒，要求实体鼠标点击和实体键盘按下/释放；仅记录各类事件数量，不保存坐标、键码、文字。超时不算通过，始终先卸载 Hook 再报告结果。
- `real_uia_hit_test_password_and_hook_service_save_retry`：创建并自动销毁 Win32 测试窗口，使用真实 WindowFromPoint、UIA 坐标/焦点反查检查普通/密码 Edit、屏幕物理 bounds 与 selector；随后真实启动/停止服务，验证重复启动拒绝、保存失败后 AwaitingSave、重试与历史读取。
- `physical_input_to_semantic_trace_and_redaction`：在专用窗口内用实体鼠标点击上方普通框并输入 `argus`，点击下方密码框并输入测试串 `secret42`，然后按 Enter 结束。完整检查两个 UIA Click、普通 TypeText、密码脱敏、Enter PressKey、零丢弃及持久化读回。只允许对该 fixture 窗口解析明文字段，其他窗口按未知字段遮盖；不会放行注入输入。要求可直接解码的英文输入，最长监听 90 秒；断言前停止 Hook 并清理测试文件。失败诊断仅打印事件类别、时序和 fixture ID，不输出文字或键码。

2026-09-07 本机已通过三轮真实 Hook 回调/过滤/卸载和原生 UIA/服务集成测试。原生 UIA 测试发现并修复了显示缩放下命中相邻控件的问题。早期仅计数的实体输入测试两次未收到鼠标点击，到期卸载；不能将它们记作通过。

用户准备好后执行了两轮完整实体输入验收：

1. 第一轮两个实体 UIA Click 通过，但普通字段的实际 `argus` 只记录为 `gus`，断言失败。用户确认文本框显示完整 `argus`，且使用中文输入法，但不记得切换时机。该轮缺少逐事件诊断，不能确认缺字根因，也不能声称已修复。
2. 增加无输入原文的逐事件诊断后，第二轮完整通过（15.98 秒）：普通字段五个字符均通过 UIA 解析并保留，密码框八个字符均 redaction，两个实体 Click、Enter PressKey、持久化读回和零丢弃断言全部通过。测试结束后 Hook、窗口及临时文件均已清理。

这证明了本机实体输入 → UIA → Trace 的一次完整成功运行，不证明中文 IME composition/commit 已支持，也不排除首轮尚未定位的缺字问题。新增键盘解码分类用于让后续异常带上明确原因。managed CDP/OCR 和桌面界面仍以先前协议/组件测试覆盖，没有进行浏览器自动化验收。

前端测试覆盖显式开始、双击去重、收起面板继续录制、停止后展示语义/原始层、保存失败重试、历史请求竞态、复制层边界、复制失败和标题栏对话框不触发窗口拖拽。没有使用浏览器自动化验收。

```powershell
cargo test -p argusflow-recorder -p argusflow-browser -p argusflow-windows -p argusflow-vision --lib
cargo check -p argusflow-desktop
node --test crates/argusflow-browser/tests/inspection-script.test.mjs
pnpm exec tsc -b --pretty false
pnpm exec vitest run src/features/recorder src/components/recorder src/components/shell/WindowTitleBar.test.tsx
pnpm exec vite build
```

只在可以使用桌面的会话内显式运行以下测试；前两条无需人工输入。第三条需要在出现 `PHYSICAL HOOK READY` 后进行实体点击和按键；第四条需要在 `PHYSICAL SEMANTIC READY` 后按测试窗口说明输入。不要使用不带用例过滤器的 `--test windows_native -- --ignored` 自动执行，因为它也会启动人工输入验收：

```powershell
cargo test -p argusflow-recorder windows_global_hooks_receive_filter_stop_and_restart -- --ignored --nocapture --test-threads=1
cargo test -p argusflow-recorder --test windows_native real_uia_hit_test_password_and_hook_service_save_retry -- --ignored --nocapture --test-threads=1
cargo test -p argusflow-recorder windows_global_hooks_capture_physical_input -- --ignored --nocapture --test-threads=1
cargo test -p argusflow-recorder --test windows_native physical_input_to_semantic_trace_and_redaction -- --ignored --nocapture --test-threads=1
```

平台语义依据：[Windows LowLevelMouseProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc)、[UIA ElementFromPointBuildCache](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-elementfrompointbuildcache)、[CDP DOM.getNodeForLocation](https://chromedevtools.github.io/devtools-protocol/tot/DOM/#method-getNodeForLocation)。
