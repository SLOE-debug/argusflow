# ArgusFlow Recorder：事件溯源的用户演示录制器

ArgusFlow Recorder 是 **event-sourced user demonstration recorder**。它采集用户实际行为与当时的证据，不在录制期间理解任务、生成 selector、合成 Click/TypeText 或提前编译 Workflow。Vision 不需要被解析成 element；没有可靠 UIA/CDP 信息时，窗口截图与坐标就是独立有效的证据。

## 三阶段边界

1. **Recorder**：鼠标、键盘、窗口切换/出现、剪贴板变化 → 按时间排序的 Event Timeline + UI evidence + screenshot evidence。
2. **多模态 AI**：录制结束后接收完整演示包，理解跨应用任务，例如“启动 xxx.exe → 复制内容 → 切换微信 → 选择联系人 → 发送复制内容”，再生成 `WorkflowDefinition`。窗口出现本身不等于已确认的进程启动；AI 应结合 EXE、窗口及前后事件理解意图。
3. **独立 compiler**：根据工作流与原始录制证据，构造 AQL/UIA/CDP/视觉执行目标，处理定位与可执行性。录制期元素 ID 只表示观察身份，不是 selector。

当前实现覆盖第一阶段及本地查看/导出。没有调用 AI API，也没有实现第二、三阶段的新生成服务。`ActionRouter` 与已有工作流执行器保持独立；Recorder 不静态依赖 browser/windows/vision 的生产实现，宿主通过 core 的只读契约装配。

## 事件与证据契约

`RecordingTrace` 使用 `schema_version: 2`，只有一个 `timeline.events`。每个事件保留：

- 捕获序号 `sequence`、Win32 原始 `timestamp_ms`、展开回绕后的 `elapsed_ms`。
- `input`：鼠标 down/up、移动、垂直/水平滚轮、键盘 down/up、窗口 `foreground`/`appeared`、剪贴板变化。
- `evidence.context`：真实窗口 HWND/PID、EXE 路径、标题、类名、物理 bounds、DPI 与浏览器 viewport 信息。
- `evidence.ui_snapshot`：可选 UIA/CDP 快照，包含来源、元素属性、祖先、bounds、字段敏感性、采样开始与耗时。不包含 selector 候选和稳定性评分。
- `evidence.screenshot`：可选 PNG 引用、采样开始与耗时、实际屏幕范围、物理像素尺寸、鼠标坐标及按下位置附近的 crop。
- 事件级与证据级诊断：延迟、窗口变更、结构化观察失败、截图失败、输入缺口与键盘遮盖。

最终按 `(elapsed_ms, sequence)` 排序。不同 Win32 事件源可能乱序送达，小幅时间倒退按历史事件处理，不误算为 49 天回绕。下游通过稳定的事件序号关联证据，不假设相邻序号必定时间递增。鼠标按下/释放、拖拽中的移动、滚轮和剪贴板事件不会因无法合成某种 Workflow 操作而消失。

### 鼠标移动合并

连续的鼠标采样会整理为一条 `pointer_motion`，独立单点保留 `move`。它是轨迹事实压缩，不是 Workflow 动作编译：保留首尾源序号、起止时间、原始采样数、原始路径累计距离、录制内观察到的按下鼠标键，以及简化后的真实轨迹点。

- 源序号连续、采样间隔不超过 200ms、诊断相同且不含独立 UI/截图证据时才合并。
- 鼠标按下/释放、键盘、窗口变化、滚轮、剪贴板事件、证据或输入缺口都会切断轨迹。先按真实时间排序，因此迟到的窗口通知也能在正确位置分段。
- 使用到线段距离的 RDP 简化，容差为 2 个物理像素，保留起终点、明显转折、折返与闭环；每约 250ms 保留真实时间锚点。累计距离按简化前全部源点计算。
- 一段最多持续 5 秒或收集 4096 个采样点，限制几何处理成本；分段/简化不增加 `dropped_events`。
- 新录制在保存前整理，历史录制在读取时使用同一幂等整理，不改写历史源文件。事件列表、分页、JSON 复制与导出均使用精简结果；历史列表计数与实际条目一致，录制时长按移动段结束时间计算。

界面一行展示一段移动，详情显示起终点、耗时、原始采样数、保留点数与路径；连续移动没有 UIA/截图时不展示误导性的“窗口信息缺失”提示。按下状态来自已观察到的鼠标事件，不能据此猜测录制开始前的按键状态。

## 采集链路

- 同一专用消息线程安装低级鼠标/键盘 Hook、`SetWinEventHook(EVENT_SYSTEM_FOREGROUND / EVENT_OBJECT_SHOW)` 与剪贴板 message-only listener。构造服务不会开始录制；停止、失败和 Drop 都卸载监听。
- 低级 Hook 只复制固定字段并非阻塞投递，始终调用 `CallNextHookEx`，忽略 injected input。窗口回调仅保留顶层窗口，附带 HWND/PID 与系统事件时间。剪贴板通知记录送达时钟与版本号。
- 摄入线程立即读取实际窗口与键盘布局、复制当前版本剪贴板，并通过 `WindowEvidenceCapture` 冻结图像。鼠标按位置找窗口；窗口事件使用事件携带的窗口身份；键盘/剪贴板使用当时焦点。
- **先冻结像素，再开始 UIA/CDP 查询**。这避免查询失败后才截图，拍到已经关闭的菜单或切换后的界面。当前对非移动事件尽可能保存完整可见窗口区域，即使之后得到 UIA/CDP 快照也保留图像。
- 截图用 DXGI Desktop Duplication 获取最近已呈现的桌面，并裁切到用户可见的窗口屏幕区域，不调用 WGC 或 OCR。包含遮挡内容，不承诺恢复离屏、最小化或被遮挡窗口内部；屏幕外边缘会裁切。负虚拟屏幕原点、跨屏拼接和旋转显示器按物理像素处理。DXGI 发布跟随桌面呈现，不承诺截图调用与显示刷新原子同步。
- PNG 压缩/写入在独立有界线程中执行，最多排队四帧。点击 crop 从同一冻结帧裁切，最大 192×192，边缘按实际图像范围缩小。不会停止录制后补拍。
- 结构化观察按 CDP → UIA 尝试，使用现有只读 runtime；不调用 Vision/OCR，不生成坐标 selector。只命中根容器不被当作可靠点击元素；窗口生命周期事件可直接记录窗口根快照。
- 普通 Chrome 不会被自动附加；CDP 仅观察当前 managed attached session。Shadow DOM 和 iframe 容器不因现有 selector 不支持而被过滤；无法确认子文档坐标变换时保留截图并报告几何缺失。

Hook 输入队列 4096，异步事件队列 2048，并发观察上限 16，整理前最多保留 100,000 个原始事件。满队列保留丢弃数和序号缺口，截图队列满则记录图像缺失。晚于事件 150ms 开始采样时不拿当前 UI 冒充过去；每个快照记录实际观察时间和耗时。结构化查询期间窗口或键盘焦点发生变化时撤销该元素，保留此前冻结的截图。

第三方 UI 无法与全局 Hook 原子冻结，菜单立即关闭、快速焦点改变、系统调度与磁盘拥塞仍可能造成证据缺失。录制器表达这种不确定性，不宣称每个事件都有严格无竞态的 UI 快照。

## 剪贴板与键盘

剪贴板变化是独立事实，支持 Unicode 文本、空、非文本、采集前再次变化和不可用状态。读取前后核对版本，绝不把后续版本内容填到之前事件。文本最多保留 65,536 个 UTF-16 单元并明确标记截断；非文本内容目前只保留变化状态，不提取图片/文件 payload。不会读取录制开始前的初始剪贴板。

键盘仍使用目标线程布局与无副作用 `ToUnicodeEx`。敏感/未知或无法证明可编辑的字段遮盖字符及可逆 vk/scan/flags，释放继承按下状态。安全的组合键保留为观察事实。**键盘脱敏不代表截图和剪贴板文本已自动脱敏**；图像与剪贴板按实际内容保存，界面明确说明这一点。

IME composition/commit 与死键最终提交文字仍无法由低级 Hook 可靠获取；保留输入事件和具体诊断，不伪造提交文字。Ctrl+C/V 与剪贴板版本可由 AI 结合时间线上下文理解，Recorder 不将它们提前改写成 Copy/Paste/TypeText 工作流动作。

## 持久化与界面

`.argusflow/recordings/<UUID>/` 是完整多模态演示包：

```text
manifest.json            # 版本、录制身份、时间、事件/截图/丢弃计数；最后发布
timeline.json            # EventTimeline，所有事件及证据引用
evidence/<sequence>.png  # 录制期间保存的可见窗口区域
evidence/<sequence>-crop.png # 可选点击局部图像
```

文件先写 `.pending` 再 rename；截图在录制期间立即保存，停止排空事件与 PNG 写入后发布时间线和 manifest。目录在安装 Hook 前创建并验证可写。生命周期为 `idle`、`recording`、`finishing`、`awaiting_save`；停止后的 JSON 发布失败可重试。强制结束进程时未发布 manifest 的目录不会出现在完整历史列表。

旧 v1 Raw/Semantic 协议不做兼容映射、双写或自动迁移；历史列表只列当前协议。没有删除用户已有录制。

控制命令为 `start_recording`、`stop_recording`、`get_recording_status`、`list_recordings`、`get_recording`；`read_recording_screenshot` 只接受录制 UUID、事件序号、window/crop 类型，通过固定文件名与路径归属检查读取 PNG，不能读取任意路径。

界面展示事件时间线、UI/窗口事实、完整截图与点击 crop；分页只影响展示，复制/导出包含完整 JSON 和证据引用。**JSON 本身不是完整多模态输入**：交给 AI 时需要一并提供 PNG，保留相对路径和事件关联。收起面板继续录制，使用“停止并保存”结束监听。录制与工作流运行互斥沿用已有控制面。

## 验证

### 截图热路径优化

`WindowsEventCapture::default()` 持有可复用的 DXGI/D3D11 会话，通过 Mutex 串行访问 immediate context，实例释放时销毁资源，避免 GPU 资源在 Windows TLS 析构边界释放。每个适配器共享设备，每个命中输出独立维护桌面复制会话；GPU 保存完整桌面，CPU staging 只分配窗口交集尺寸。每次返回独立 BGRX 字节，不逐像素换色或降低分辨率。截图前后复验 HWND/PID、可见性和边界。

首帧必须取得非零 `LastPresentTime` 的真实桌面呈现，不能把鼠标通知的未初始化纹理当成功截图。后续只在 DXGI 确认没有桌面更新时复用图像，同一区域可跳过重复 GPU 传输。显示器断开、移动、旋转或复制会话失效会撤销缓存并报错，下次调用重建；没有 GDI 回退。首次呈现等待和 GPU 映射轮询分别有 100ms 预算，但设备/复制会话初始化不受此预算约束。

后台 PNG 线程消费帧并原地转换为不透明 RGBA，再保存完整图和 crop；编码使用无损 `Fastest` 档位。core、recorder、windows 及 PNG 热路径依赖在开发构建中开启优化。150ms 的历史证据约束未放宽。

历史 GDI 优化曾在 2560×1549 的 10 次预热样本中得到 193.285ms → 32.539ms。2026-09-07 新基准改为自有动态窗口、四象限逐帧变色、4 帧预热和 100 次采样，完整截图包含身份校验、GPU 读回和独立像素复制；绘制、呈现等待、PNG 编码不计入截图耗时。每轮验证颜色、方向及上一帧未被覆盖，错误直接终止，不剔除失败样本。

本机开发构建，同窗口顺序对照（DXGI 后 GDI）：1920×1080 中位数 26.242ms → 6.104ms，DXGI P95 11.862ms；2560×1440 中位数 36.215ms → 14.182ms，DXGI P95 15.137ms。每组 104 次动态校验和 103 次独立帧校验通过。冷启动分别 272.212ms / 264.663ms，未达到个位数；大尺寸动态截图及长尾也尚未达到个位数。结果受驱动、负载和刷新时序影响，不把静态缓存样本宣传为动态性能，也不与旧尺寸直接算加速比。未执行真实用户输入录制验收。

模块整理后追加复测：1920×1080 中位数 9.511ms、P95 10.695ms；原始尺寸 2560×1549 同轮 GDI 37.138ms → DXGI 15.149ms（约 2.45 倍），DXGI P95 17.292ms、最大 18.307ms，冷启动 234.067ms。两组动态及独立帧校验仍全部通过。1080p 多轮中位数约 6–9.5ms，原始大窗口尺寸尚未达到个位数，不能将最初静态窗口的 2.63ms 当作完整动态截图结论。

```powershell
# 只输出完整截图耗时，不安装 Hook，不保存屏幕图像。
cargo run -p argusflow-windows --example recorder_capture_bench
# 自有动态窗口，不夺取焦点；必须能完整放进主屏幕。
cargo run -p argusflow-windows --example recorder_capture_bench -- --fixture 1920 1080
# 可选：同一画面同时对照旧 GDI 路径，仅影响基准程序。
$env:ARGUSFLOW_BENCH_GDI = '1'
cargo run -p argusflow-windows --example recorder_capture_bench -- --fixture 2560 1440
Remove-Item Env:ARGUSFLOW_BENCH_GDI
# 仅合成图，输出后台转换/编码/写盘耗时，清理临时 PNG。
cargo test -p argusflow-recorder full_size_encoding_benchmark --lib -- --ignored --nocapture
```

常规测试使用纯数据、模拟 provider 和临时 PNG，覆盖事件顺序、时钟回绕/乱序、窗口/剪贴板事实、焦点变化脱敏、结构化失败后保留冻结图像、负原点 crop、PNG/时间线持久化和受限证据读取。组件测试覆盖生命周期、事件 JSON、证据预览与异步 URL 清理；不自动运行浏览器或安装真实全局 Hook。

```powershell
cargo test -p argusflow-recorder -p argusflow-browser -p argusflow-windows --lib
cargo test -p argusflow-recorder --no-run
cargo check -p argusflow-desktop
node --test crates/argusflow-browser/tests/inspection-script.test.mjs
pnpm exec tsc -b --pretty false
pnpm exec vitest run src/features/recorder src/components/recorder src/components/shell/WindowTitleBar.test.tsx
pnpm exec vite build
```

真实桌面验收保留为显式 `#[ignore]` 测试：`real_uia_hit_test_password_and_hook_service_save_retry`、`physical_input_to_event_timeline_and_evidence`。后者需人工输入固定 fixture 文本；不得作为普通回归自动执行。以前 Semantic Recorder 的实体输入验收不能当作新事件/截图链路已通过真实桌面验收的证明。

平台依据：[剪贴板监听与版本](https://learn.microsoft.com/en-us/windows/win32/dataxchg/using-the-clipboard)、[SetWinEventHook](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook)、[BitBlt](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-bitblt)。
