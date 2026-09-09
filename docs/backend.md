# 后端能力接口与生命周期

## 共用契约

通用动作、操作票据和错误基础类型属于 `argusflow-core`；采样领域契约集中在 `argusflow-capture-contracts`。两者不依赖运行时。Windows、Capture 和 Vision 通过采样契约连接，平台后端不依赖 OCR，OCR 不依赖 Windows 或采样服务实现。异步服务使用 Tokio。UIA/OCR/真实输入在各自专用原生线程执行，CDP 使用独立收发任务和一个拥有请求表的 actor。

`OperationOptions::new(Duration)` 接受 0 到 24 小时之间的非零总时限。截止时间从调用开始计算，覆盖排队、连接和所有后续步骤，不因为重试重置。Future 被丢弃会取消排队和后续步骤。原生调用只承诺协作取消；超时不等于底层调用已经退出。

| 资源 | 默认限制 |
| --- | --- |
| 普通调用 / 启动浏览器 / 模型加载 / OCR | 10 / 30 / 120 / 60 秒 |
| UIA 等待队列 | 64 |
| UIA 遍历 / 深度 / 返回和存活租约 | 10000 / 64 / 256 |
| UIA 租约 / Provider 连接 / Provider 事务 | 60 / 2 / 5 秒 |
| 真实输入等待队列 | 64 |
| CDP 排队加等待响应 | 128 |
| CDP 消息 / 事件缓冲 / 写入超时 | 8 MiB / 128 条 / 5 秒 |
| 附加页面 / 每次 CSS 结果 | 64 / 256 |
| 自建浏览器（含尚在清理的实例） | 8 |
| OCR 每个引擎等待队列 | 2，逐张、逐文本区域推理，识别 batch 固定为 1 |
| OCR 编码图片 / 解码图片 / 检测最长边 | 64 MiB / 1600 万像素 / 4000 |
| OCR 文本候选 / 单个边界点 | 3000 / 200000 |

队列满立即返回 `Busy`。同进程 UIA 和真实输入分别只允许一个服务，使用 Clone 共享；OCR 每个 `(ModelTier, Device)` 只允许一个引擎。占用由原生线程持有直到清理完成，初始化超时后重复创建不会不断堆积线程。

`WindowsError`、`BrowserError`、`OcrError` 为能力各自的错误类型。`kind()`、`stage()`、`request_id()`、`resource()`、`effect()` 可用于处理与诊断，`std::error::Error::source()` 保留原始上下文。`Effect::Unconfirmed` 表示已进入副作用调用但无法确认最终结果；不能把超时、断连或失败当成动作没发生，更不能自动重放。错误分类不等于重试策略。

默认 tracing 不记录输入文字、识别内容、脚本或图片。显式打印原始错误 source 可能包含对端诊断，应由应用控制。各服务提供只读状态与 `shutdown`；在 Tokio runtime 仍存活时显式关闭。原生线程卡住时 shutdown 返回 `Unresponsive`，资源和实例占用仍保留。原生崩溃和永久卡死没有进程隔离级恢复保证。

## Windows

`WindowLocator` 按 PID、精确标题、精确类名筛选可见顶层窗口，`find_unique` 必须唯一。`WindowIdentity` 保存 HWND、PID、进程创建时间，并安装一个属于本库的窗口属性租约；属性随窗口销毁消失，最后一个身份句柄释放时移除。使用前复验标记，阻止同进程 HWND 复用；标记被移除后旧身份不会重新绑定。最多同时持有 4096 个窗口身份。目标权限不允许安装标记时明确报错，不降级校验。窗口枚举与身份建立是同步 Win32 元数据操作；不会切换焦点。目标仍可能在校验后变化，真实输入不是事务。

`UiaRuntime::start` 创建专用 MTA；COM 元素、Pattern、TreeWalker 均在该线程创建和释放。`Query` 由窗口身份、`SearchScope` 和 `Predicate` 组成；条件支持 AutomationId、Name、ClassName、ControlType、All、OneOf、Not。查询不跨越指定窗口树，达到预算返回错误，不把截断结果伪装为完整结果。

`find_all/find_unique` 返回租约句柄；`read` 返回拥有所有权的属性快照。句柄绑定 UIA 实例，显式 `release` 会撤销所有克隆；租约到期、窗口失效、元素脱离树或 Provider 报失效后不可使用。

`perform` 支持 Invoke、SetValue、Focus、Toggle、Selection（选择/添加/移除）、Expand、Collapse、Scroll、ScrollIntoView。缺少 Pattern 返回 `Unsupported`；不会隐式改成坐标点击。

`InputService` 独立提供 Move、Click（左右键、单双击）、Wheel（水平/垂直）、Text、Chord。鼠标坐标是屏幕物理像素，接受负坐标；按当前虚拟桌面尺寸归一化并在工作线程启用 Per Monitor V2 DPI 上下文。目标窗口必须已在前台，点击点必须在目标窗口且没有被其他顶层窗口遮挡。不会自动激活窗口。组合键拒绝重复键和用户已经按住的键。

SendInput 一次性提交有限事件序列并核对实际注入数量。部分注入只释放已注入序列中仍然按下的键；不重放按下或文本。UIPI 权限不匹配会明确失败，库不提升权限。

手动示例：`cargo run -p argusflow-windows --example test_window`，另一个终端执行 `cargo run -p argusflow-windows --example uia`。后者只对精确标题为 `ArgusFlow UIA Test` 的测试窗口调用 Invoke。

## CDP

`Browser::connect` 接受显式 HTTP(S) 调试端点或 Browser WS(S) 地址。`Browser::launch` 接受 Chrome/Edge 的绝对路径，用随机本地端口和独立临时配置目录启动；通过 `DevToolsActivePort` 获知端口，不扫描用户端口或使用日常配置目录。Windows 自建进程关联 Job，关闭时回收自有进程树和临时目录。

`pages` 列举 target；`attach` 附加指定 target，重复附加复用 session。页面装配有排他额度，忙时返回 Busy。`new_page` 在独立且 `disposeOnDetach` 的浏览器 context 中创建，因此不共享外部页面的 cookie，断连时回收自建页面。外部浏览器只断开连接和脱离 session，不发送 Browser.close；不会关闭外部已有页面。

Page 支持 navigate、find_all/find_unique、evaluate、insert_text、press、wheel、close、detach。导航等待新主 frame 的 loader 提交及 document.readyState 为 interactive/complete，不等待网络空闲。Element 支持 read、focus、scroll_into_view、click。

CSS 只查询主文档，不穿透 iframe 和 Shadow DOM。多个匹配不会选择第一个。元素绑定文档代际，DOM.documentUpdated、主 frame 导航、执行上下文清除、session 销毁和断连使旧句柄失效。属性读取短期使用 objectGroup，并在取消或完成时有界释放。

输入在同一浏览器连接内排他执行；按键清理完成前不接受另一组输入。副作用命令不重试；补偿只发送 release。清理也占用有界额度，清理失败时停止连接，避免积累未知远端资源。

WebSocket 读写相互独立，写入阻塞受自己的时限和操作剩余时间约束。超时、取消、关闭和 session 销毁清除待响应请求；迟到响应被丢弃。创建资源的响应丢失时断开连接，依赖 session/context 的连接所有权回收。

手动示例：`cargo run -p argusflow-browser --example cdp -- http://127.0.0.1:9222`。该例只创建并操作自己的临时页面，最后断开外部浏览器连接。开发期间不自动执行该例。

## OCR

`OcrConfig::new(dependencies)` 默认 Small + Cpu。显式选择 `ModelTier::Medium` 或 `Device::Cuda { device_id }` 后不自动切换档位或设备。`OcrEngine::load` 校验固定官方模型与配置 SHA-256，加载检测和识别 Session。识别时不会下载资源。

`ImageInput` 接受 Path、Encoded、Pixels 和 Shared。Pixels 必须声明宽高、stride、Rgb/Bgr/Bgrx/Rgba/Bgra 格式；真正透明像素以白色合成，Bgrx 的第四通道完全忽略。Shared 直接持有采样契约的只读 `PixelImage`，不经过编码、临时文件或 Python。编码格式支持 PNG/JPEG/BMP/WebP，先检查格式尺寸再解码。

检测使用 BGR、官方均值/标准差、短边 736、32 倍数缩放，DB 概率阈值/框阈值/外扩来自所选模型 inference.yml。有界连通域提取外边界，最小旋转矩形评分及外扩后转换回原图。读取顺序先按行再按行内位置排序。透视裁剪后竖长区域旋转；识别输入高 48、最小宽 320、最大宽 3200，BGR 归一化到 [-1,1]，CTC 字典来自对应官方配置。极长单行文本会压缩到宽度上限。

结果包含文字、平均保留字符置信度和原图四边形，`text()` 用换行拼接。无文字返回空结果。没有方向分类、去畸变或版面分析；本实现不声明与 OpenCV 的逐像素插值/轮廓顺序完全相同，准确性由固定图片验收覆盖。

`SampledOcr` 将指定区域的稳定采样接到现有引擎。每个实例固定模型配置，容量为八个区域，合并相同区域的在途调用；只在 GPU 精确确认完整区域内容不变后复用结果。结果保留来源版本、本地原点和屏幕坐标转换，区域外变化不重复推理。详见 [采样设计](sampling.md)。
