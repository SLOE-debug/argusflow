# ArgusFlow 微信自绘 UI 视觉感知 / PaddleOCR v3.7 实施方案

- 仓库：`SLOE-debug/argusflow`。
- 分析基线：`main @ 6ffa38ad8d1a8b9587044cc493add8b959ed2944`（2026-08-28）。
- PaddleOCR 基线：`PaddleOCR 3.7.0` + `PP-OCRv6`（官方 2026-06-11 发布）。
- 目标：补齐微信这类 Qt / GPU 自绘 UI 的视觉感知、验证与智能滚动，同时保持现有 AppSession / PreparedPlan / Backend / Evidence 架构。
- 范围：Windows-only；默认本地 OCR；默认不抓整个桌面；默认不把截图持久化。
- 本文重点：只捕获目标应用、差分 ROI、文本层、VisualScene、Owned Popup、视觉验证、智能整页滚动、无旧数据污染。

## 0. 结论先行

- 不要把 ArgusFlow 改成“全屏截图 + OCR + 坐标点击”的机器人。
- UIA / CDP 继续做语义快路径；Vision 是像素事实层、fallback 与 post-condition 验证层。
- 微信内部 UIA 不可见时，Vision 可以成为主观察通道；执行仍优先使用快捷键、SendInput、可用的 UIA Pattern。
- 捕获对象不是“exe 名字符串”，而是已经解析出的 `AppSession + HWND + PID + 动态 WindowSet`。
- OCR 不应每帧全图运行；应维护稳定帧、Dirty Region、VisualScene cache，只重识别变动 ROI。
- PP-OCRv6 tiny 用于高频增量识别；PP-OCRv6 medium 用于低置信度升级与关键验证。
- `\t / \n / 空格` 拼出来的文本层只是一种投影；真正的事实模型必须保留 bbox、confidence、region、frame/scene generation。
- 智能滚动不能按固定 wheel delta 定义“一页”；应按实际视觉位移定义一页，并要求新旧页存在可验证 overlap。
- CurrentViewportScene 与 ScrollDocumentHistory 必须分开，从数据模型上杜绝滚动后的旧信息残留。
- 发送消息是非幂等动作；发送后视觉验证失败时不得盲目自动重发。

```text
UIA / CDP -> VisualCache -> OcrTiny -> OcrMedium -> GuiGrounding
                  |                         |
                  +---- VisualScene -------+
                              |
                 CompactText / SpatialText
                              |
                      Verify -> SendInput
                              |
                        Verify again
```

## 1. 对微信 UIA 现象的工程判断

- 顶层类名 `Qt51514QWindowIcon` 与 Qt 5.15 外壳一致。
- `MMUIRenderSubWindowHW` 更像硬件渲染承载子窗口，而不是标准 Win32/UIA 控件树。
- 内部可能是 Qt Scene Graph、DirectComposition、D3D 纹理或微信自研渲染层；具体实现并不影响本方案。
- 对 UIA 而言关键事实是“像素可见，但 Accessibility fragment 不存在”。
- 若没有 `IRawElementProviderFragment` 等 provider，`FindAll` / `GetFirstChildElement` 不可能凭空生成内部按钮和文字。
- 搜索、菜单、对话框仍可能产生独立 HWND 或 Owned Popup，因此必须动态枚举 WindowSet，不能只盯主 Pane。
- `WM_GETOBJECT / IAccessible` 值得做一次 capability probe，但不应阻塞 Vision；没有 Legacy pattern 时完整 MSAA 树概率较低。
- 这类窗口应正式分类为 `Semantic Tree Opaque Surface`：窗口身份可见、像素可见、内部语义树不可见。
- 不要把 `Qt51514QWindowIcon` 或 `MMUIRenderSubWindowHW` 写成永久 selector；它们只用于诊断 evidence。

## 2. 与当前仓库结构的对齐

- 现有 `AppSession` 已经解决应用实例、窗口、PID、启动/附加/恢复等资源问题。
- 现有 `AutomationExecutionScope::Window` 已能冻结 HWND/PID 与 capability。
- 现有 `ActionRouter / PreparedCandidate / PreparedPlan` 是 Vision 必须复用的执行骨架。
- 现有 `BackendKind` 已预留 `VisualCache / OcrTiny / OcrMedium / GuiGrounding / SendInput`。
- 现有 `argusflow-vision` 仍是 NotImplemented 占位，正适合填实而不是重写顶层 runtime。
- 现有 `argusflow-windows::capture` 中 WGC / DXGI 也是占位，需先实现真实捕获。
- 现有 AppSession capability 已包含 `VISUAL_SCREEN`，只需让能力从声明变成真实 runtime。
- 现有 Failure Evidence 已有 `Screenshot / OcrRegions / OcrOverlay` artifact 类型。
- 现有 target wait 已统一到 PreparedPlan；Vision target miss 也应复用，不在 OCR backend 偷偷 sleep。
- 最新微信 commit 已打通 `PressKey / TypeText / SendInput`，Vision 第一价值应是补“动作前确认 + 动作后验证”。

## 3. 总体分层

```text
L0 Resource Identity
   AppSession / WindowIdentity / PID / HWND
L1 Native Semantic
   UIA / CDP
L2 Visual Fact
   WGC -> Diff -> OCR -> VisualScene
L3 Expensive Visual Reasoning
   Medium retry / GUI grounding / future VLM
```

- Observe、Decide、Act、Verify 分层，避免“OCR 找到文字就直接点”。
- UIA 可以成功时继续优先 UIA；微信 opaque surface 才提高 Vision ContextFitness。
- Vision 同时承担 fallback 与 verification，但不要求每个低风险 UIA Click 都强制视觉验证。
- 高风险非幂等动作应使用 Required verification，例如发消息、提交、删除。

## 4. 目标应用作用域：用户说 exe，内部冻结 AppSession

- 产品上可以表达“仅识别 WeChat.exe”。
- 运行时必须转换成 `AppSession + primary HWND + PID + allowed dynamic windows`。
- exe 名只用于发现/校验，不是执行期唯一身份。
- 同 exe 多实例时必须沿用 AppSession 的唯一性规则，不允许 Vision 自己选“最像”的窗口。
- HWND 会复用，因此每次关键操作都要 `HWND -> PID` 复验。
- Owned Popup 默认只加入同 PID / 同会话允许的窗口，避免 OCR 其他应用或 IME 候选窗。
- 跨进程 helper window 后续可做，但必须显式归属 AppSession，不按标题模糊扩域。
- 系统文件选择器属于另一 scope，不应为了它把微信 Vision 放宽到全桌面。

## 5. 捕获后端

- Primary：`Windows.Graphics.Capture` 按 HWND 持续捕获。
- Fallback：DXGI Desktop Duplication，仅在 WGC 不可用或特殊诊断场景使用。
- 默认不使用全桌面 OCR，既降低隐私暴露也减少计算量。
- 传统 PrintWindow / BitBlt 对 GPU 自绘窗口可黑屏/空白/旧帧，只做低优先级诊断 fallback。
- 窗口最小化、关闭、resize、重建时捕获流要显式进入 degraded/reopen，而不是继续复用旧 scene。
- WGC 对遮挡、最小化等具体行为应通过 Windows integration test 固化，不写未经验证的契约。
- 建议保留 capture health：黑帧、近均匀帧、预期变化却长期同帧、frame timeout。

## 6. 动态 WindowTopologyTracker

```text
AppSession primary HWND
   -> EnumWindows same PID
   -> owner chain
   -> visible/non-zero windows
   -> Primary / OwnedPopup / SameProcessTopLevel
   -> topology_generation++ on changes
```

- Ctrl+F、菜单、对话框等动作后主动 refresh topology。
- P0 可 polling；P1 可用 WinEvent hook 仅做提前唤醒，正确性仍靠 refresh + identity validation。
- Popup 出现/消失必须改变 topology generation，并让相关 scene 节点失效。
- 若 popup 是同一 GPU surface overlay，topology 不变，Dirty Region 会检测到局部大变化。
- 上层应统一抽象为 `VisualSurface`，不关心 popup 是独立 HWND 还是同 surface overlay。

## 7. Frame / Surface 数据契约

```text
CapturedFrame {
  frame_id, topology_generation, hwnd, process_id,
  timestamp_qpc, width, height, dpi_x, dpi_y,
  pixel_format, content_rect, storage
}
VisualSurface { surface_id, window, frame, relation }
```

- 所有 OCR response 必须回带 frame_id 与 ROI。
- 异步 OCR 结果若 generation 已过期，禁止写入 current scene。
- 每个 bbox 明确属于 FrameLocal / ClientPhysical / VirtualScreenPhysical 中哪一套坐标。
- DPI / client-to-screen 转换放 `argusflow-windows`，Vision 不自己猜 scale。

## 8. 差分 ROI：高性能核心

- 维护 `PreviousStableFrame -> CurrentFrame -> DirtyMap -> VisualSceneCache`。
- 先将 BGRA 缩小到 1/4 或 1/8，并转灰度/亮度，再做 abs diff。
- 以 32x32 capture pixel 左右的逻辑 tile 聚合变化，阈值全部可配置。
- 相邻 dirty tile 做 connected merge，再向外 padding 12~24px，避免把文字切断。
- ROI 过多时直接合并/升级为区域级或 full refresh，避免几十上百个 OCR request。
- 当 dirty coverage 超过初始建议 0.35，视为 major transition，优先 full region/window OCR。
- 打开聊天、resize、DPI change、popup topology change、大幅 scroll 都属于 major transition。
- “只识别变动 area”是优化策略，不是教条；大变化时 full refresh 反而更正确。
- 记录 `ocr_processed_pixels / captured_pixels`，用数据证明增量识别是否真正节省。

## 9. Temporal Noise Mask 与 StableFrameGate

- 输入 caret、typing indicator、呼吸动画可能导致某个小区域周期变化。
- 对固定小区域的周期 toggle 建立短 TTL temporal noise mask。
- 如果同区域出现真实新文字，必须立即逃逸 mask，不得永久忽略。
- 滚动/动画过程中不要直接 OCR；先要求连续 2~3 帧有效 dirty ratio 低于稳定阈值。
- Stable wait 必须有 deadline，例如初始 800ms，可按场景调整。
- Visual stability wait 与 Target readiness wait 是两件事：前者由 Vision runtime 管，后者由 PreparedPlan 管。
- 若 deadline 内始终不稳定，返回明确 `FrameUnstable` / 映射错误，不无限等待。

## 10. PaddleOCR 选型

- 截至基线，官方最新 release 是 PaddleOCR 3.7.0，核心新增 PP-OCRv6。
- PP-OCRv6 有 tiny / small / medium 三档，官方给出的参数量约 1.5M / 7.7M / 34.5M。
- medium/small 单模型覆盖 50 种语言，适合桌面中英日及多语言 UI。
- 现有 `BackendKind::OcrTiny` 对应 PP-OCRv6 tiny，承担高频 dirty ROI。
- 现有 `BackendKind::OcrMedium` 对应 PP-OCRv6 medium，承担低置信度升级和关键安全验证。
- 不需要为了 PP-OCRv6 small 立刻扩 Core enum；Core backend 表达能力层级，不应绑死厂商 SKU。
- PaddleOCR-VL 更偏文档理解，不应作为 GUI 文本默认路径；GUI 需要忠实 bbox、低延迟、低幻觉。
- GUI grounding 只作为纯图标/无文字控件等最后兜底。
- 桌面 GUI 默认关闭不必要的 document orientation / unwarping 等文档预处理，保留 adapter 以 3.7.0 实际 API 为准。

## 11. OCR 升级策略

```text
VisualCache hit -> return
else Tiny OCR dirty/query ROI
  -> confidence + uniqueness sufficient -> return
  -> otherwise expand/upscale same ROI
  -> Medium OCR
  -> still ambiguous -> explicit failure / grounding fallback
```

- 关键 header / send post-condition 可直接请求 medium，不必先 tiny。
- 低置信度重试可以扩大 ROI、2x upscale 小字体，再调用 medium。
- 阈值不要写成产品契约，必须用真实微信 fixture benchmark 校准。
- 保留 raw_text 与 normalized_text，不允许语言模型静默“纠正”OCR 后当成事实。

## 12. 独立 Python Vision Worker

- 新增 `workers/argusflow-vision-worker/`，不要把 Paddle runtime 嵌入 Tauri/Rust 主进程。
- 锁定 `paddleocr==3.7.0`；inference engine 同样锁版本。
- 官方文档说明 paddleocr 本体支持 Python 3.8+，生产仍应选择经过 ArgusFlow 验证的固定 Python 版本。
- 一个 worker venv 只放一种主要 inference engine，避免 PaddlePaddle / ONNX / Transformers 混装冲突。
- worker 生命周期：Starting -> LoadingModels -> Ready -> Degraded/Failed。
- health 回报 protocol_version、worker_version、paddleocr_version、model、device、engine。
- 模型 lazy load：tiny 首次 Vision 时加载，medium 首次升级验证时加载；可提供 prewarm setting。
- worker crash 有 bounded restart，禁止 crash loop 无限重启。
- worker Ready 前做中文/英文小图 warmup，确认 detection/recognition 真能工作。

## 13. IPC 与像素传输

- Control plane 用 Windows Named Pipe；避免默认 localhost HTTP 端口。
- 消息可用 framed JSON/MessagePack，必须有 request_id、deadline、error code、protocol version。
- Pixel plane 主路径用 named shared memory / file mapping，避免把 4K BGRA base64 塞 JSON。
- P0 可保留 InlineBytes 仅用于小 ROI、测试、debug。
- 共享内存必须使用 lease / ring slot，worker ack 或 timeout 后 Rust 才复用。
- Named Pipe / mapping 使用当前用户 ACL + 随机 session token。
- worker queue 有硬上限；只保留最新相关 generation，旧请求取消/丢弃。
- 高风险 verification request 优先级高于后台 cache refresh。

## 14. OCR 协议最小字段

```text
ocr_request:
  request_id, profile, frame_id, hwnd, pid, roi, pixel transport, options
ocr_response:
  request_id, frame_id, model, elapsed_ms, items[]
item:
  raw_text, confidence, polygon
```

- Rust 在 merge 前复验 frame_id、topology generation、window identity、request cancellation。
- worker 输出 polygon；Rust 统一计算 bbox/center/line geometry，避免两套坐标算法。
- 同一 frame 多个 ROI 可以 batch，减少 IPC 与模型调度开销。

## 15. VisualScene：真正的事实模型

```text
VisualScene {
  scene_id, frame_id, topology_generation, window, viewport,
  regions[], nodes[], compact_text, spatial_text
}
VisualNode {
  id, generation, raw_text?, normalized_text?, role_hint, bbox,
  polygon?, confidence, source, region_id?, line_id?, row_id?, stable_hash
}
```

- OCR 纯字符串不能表达它在联系人栏、聊天 header 还是消息正文。
- `text + geometry + generation` 三者缺一不可。
- `VisualNodeSource` 至少区分 OcrTiny / OcrMedium / LayoutHeuristic / UiaProjection / GuiGrounding。
- 相邻 scene 可通过 text + bbox proximity + region + patch hash 维持短期 logical stable id。
- logical id 只用于短期 tracking，不可成为跨应用重启的永久 selector。
- Scene semantic delta 应提供 added / removed / changed nodes，消息发送验证可直接查询 added nodes。

## 16. Scene 缓存与失效

- `VisualCache` 代表最近稳定 VisualScene，不是最近一张 PNG。
- cache 命中至少要求 same window、compatible topology generation、scene freshness、query region 未 dirty。
- Dirty ROI 只 invalidates 相交节点，未变化的 sidebar/header 可继续复用。
- 新的 OCR result 只重建受影响的 line/row/region clustering。
- major transition 直接生成新 full scene，避免复杂碎片合并。
- 旧 OCR result 即使晚到也只能进 metrics，不能写 current scene。

## 17. CurrentViewportScene 与 ScrollDocumentHistory 必须分开

- CurrentViewportScene 只包含当前屏幕上可见节点。
- 滚动后 current scene 是新稳定帧的结果，不与旧页做加法合并。
- ScrollDocumentHistory 用于主动分页采集的历史拼接，允许保存已经离屏的内容。
- History append 必须基于 overlap anchor 去重。
- History 绝不能反向污染 current scene。
- 这一条从数据模型层解决“翻页后仍残留旧信息”的问题。

## 18. CompactTextProjection

- 用途：日志、规则、LLM context、人工理解、Evidence。
- 同一视觉行合并为同一文本行。
- 普通词间 gap 用空格。
- 跨明显稳定分栏的 gap 用 `	`。
- 跨视觉行用 `
`。
- 跨明显 region 可插入空行或 `[Header] / [Sidebar]` 等开发者 marker。
- 中文不能仅用 ASCII 字符数估宽；结合 bbox 与 Unicode display width。
- 输出必须 deterministic，相同 scene 得到相同字符串。

```text
[联系人]
项目讨论群	10:21	张三：收到
自动化测试群	09:48	李四：已更新

[聊天：项目讨论群]
张三	10:18
今天部署吗？
我	10:19
可以，下午三点。
```

## 19. SpatialTextProjection

- 用途：近似还原应用“面貌”、debug layout、给模型低成本空间上下文。
- 按 viewport 宽度归一到固定列数，例如初始 120 列，不按真实字体 pt 强行映射。
- `col = round(x / viewport_width * spatial_columns)`；row 来自视觉 line clustering。
- 中文双宽字符、不同字体、DPI 都只影响近似显示，不影响 bbox 真值。
- 冲突时优先高置信度/最新 generation；不得为了排版漂亮修改 OCR 文本。
- SpatialText 不能保存成永久 selector；窗口大小/DPI/字体变化会改变 spacing。

## 20. Layout / Region

- VisualScene 不应只是一张平面图，应有 Navigation / Sidebar / List / Header / Content / ChatHistory / Editor / Popup / Dialog / Unknown 等 region。
- region 可由稳定分割线、多帧变化模式、OCR 对齐、重复 row、UIA 外层 bbox、AppProfile 弱先验共同推断。
- Generic region inference 必须先于微信特判。
- WeChat profile 只提供“左侧列表、顶部 header、底部 editor”等弱先验，不含绝对坐标。
- 多帧 tile change_frequency 可以学习 StaticChromeMask / DynamicContentMask。
- UIA 即使看不到内部元素，外层 Pane bbox 仍可作为视觉 region boundary hint。

## 21. VisualQuery / 查询安全性

- 当前 `VisualQuery { text, exact }` 足够 P0，不需要这次重做 AQL。
- 先实现 exact/fuzzy text -> current VisualScene -> 0/1/N。
- 0 -> TargetNotFound；1 -> Ready；N>1 -> AmbiguousTarget。
- fuzzy 只负责产生候选集，不允许“最高分就是目标”的隐式选择。
- 微信群名和聊天 header 应优先 exact。
- P1 可增加 region / role_hint / relation hint，但保持结构化、可 Explain、可 deterministic evaluate。
- 长期可让 portable AQL compiler 生成 Vision candidate，但 P0 不改 grammar。

## 22. Planner / Backend 路由

- 相对顺序建议：UIA/CDP native -> VisualCache -> OcrTiny -> OcrMedium -> GuiGrounding。
- `VisualCache` cost 低；Tiny 中；Medium 高；Grounding 最高。
- AppSession 有 VISUAL_SCREEN 且 scene fresh 时，提高 VisualCache ContextFitness。
- UIA 被诊断为 opaque surface 时，可提高 OCR ContextFitness，但不按 class name 写死。
- worker 未 Ready 时必须是 RuntimeAvailability != Ready，不能用 cost 掩盖。
- SendInput 是 actuation capability，不应被简单理解成 selector backend 的下一档。

## 23. PreparedPlan 语义

- prepare 阶段冻结 query、backend kind、HWND/PID、capture runtime logical handle、worker profile、branch_path、diagnostics。
- prepare 不冻结 OCR bbox；bbox 是 materialize/execute 的瞬时事实。
- execute 时复验窗口身份、获取最新稳定 scene、必要时刷新 dirty/query ROI，再执行 0/1/N 查询。
- Vision TargetNotFound 进入现有 PreparedPlan bounded target wait。
- 不要在 OcrBackend 内部写隐藏 5s poll；这会破坏统一 deadline 与 Explain。
- Visual stability wait 可以在 backend/runtime 内部，因为它是像素采样可靠性，不是“等业务 target 出现”。

## 24. VisualCacheContext 建议

```text
VisualCacheContext {
  ready,
  capture_ready,
  worker_ready,
  scene_available,
  scene_age_ms?
}
```

- 当前只有 `ready: bool` 太粗；可先扩内部 runtime health，再决定是否升级稳定序列化结构。
- “能截图”不等于“Vision backend Ready”；至少还需要可用 scene 或 OCR worker。

## 25. 点击 / 输入：视觉观察与物理执行分开

- 当前键盘 SendInput 已实现，鼠标点击/滚轮尚未实现，是完整视觉操作的后续缺口。
- 视觉层输出 target bbox / safe point / scroll region，不直接调用 User32。
- `argusflow-windows::input` 实现 mouse / wheel，并继续做 foreground + HWND/PID 验证。
- 视觉 bbox 从识别到点击之间可能过期；点击前必须 revalidate target region。
- 若 target region dirty，重新 tiny/medium OCR 获取最新 bbox。
- List row 可推断完整 row hit rect；不要长期依赖“文字 bbox center + 魔法偏移”。

## 26. Verification 抽象

```text
VerificationOutcome = Confirmed | Rejected(reason) | Uncertain(reason)
VisualCondition:
  TextExists(text, exact, region?)
  NewTextExistsSince(text, since_scene_id, region?)
```

- 微信需要的第一批 verification：搜索 UI 出现、搜索结果 exact、聊天 header exact、新发送消息出现。
- 高风险动作的 Uncertain 不应自动继续。
- 后续可抽象为 `VerificationPolicy::None / BestEffort / Required`，P0 可先在微信流程局部 required。

## 27. 微信群搜索与发送：推荐闭环

1. Acquire WeChat AppSession。
2. 捕获 primary window 并生成初始 stable VisualScene。
3. 通过 SendInput 按 Ctrl+F。
4. 等待 frame/topology change。
5. 等待视觉稳定。
6. OCR 变化的 search overlay/result region。
7. 确认 search state 已出现。
8. TypeText(group name)。
9. 等待搜索结果区域变化并稳定。
10. OCR 搜索结果 rows。
11. 聚合 row primary/secondary text。
12. 要求 intended group exact 唯一命中。
13. 若键盘语义稳定则 Enter；否则后续用视觉点击 row。
14. 等待 major transition stable。
15. OCR Header region。
16. 要求 Header exact == intended group。
17. 只有 header 确认后才允许发送。
18. TypeText(message)。
19. 可选 strong mode：OCR Editor region 验证 draft。
20. 记录 before_send_scene_id。
21. Press Enter。
22. 等待 ChatHistoryBottom dirty + stable。
23. Medium OCR bottom region。
24. 查找 `NewTextExistsSince(message, before_send_scene_id)`。
25. 结合 bottom region + outgoing alignment hint。
26. Confirmed 才返回成功。
27. 若 timeout -> Uncertain，不重复 Enter。

## 28. 为什么要搜索结果 + Header 两次确认

- 只确认搜索结果：Enter 仍可能因为 focus/排序变化进入错误聊天。
- 只确认 Header：错误进入后虽然会拦发送，但无法解释搜索结果阶段发生了什么。
- 两次确认让“选择目标”和“实际进入目标”成为两个独立事实。
- 同名文字可能同时出现在左侧联系人、header、消息正文，所以 Header query 必须加 region。

## 29. 发送后验证与重复消息风险

- 发消息是非幂等动作。
- 不能只做 `TextExists(message)`，因为历史里可能已经有同样文本。
- 至少使用 `NewTextExistsSince(before_send_scene_id)`。
- 再加 `ChatHistoryBottom` region；P1 可加 outgoing/right alignment hint。
- 长消息先按 message block 聚合多行，再匹配 normalized block text。
- 验证超时只能返回 Uncertain/明确错误，不得默认再次按 Enter。

## 30. 智能滚动：一页的正确定义

- 不同应用/系统设置的 wheel 行数不同，固定 `wheel=-720` 无法保证一页。
- 定义 viewport height = H，preferred overlap 初始建议 0.18H。
- 目标内容位移 `target_shift = H * (1 - overlap)`，例如约 0.82H。
- 滚动控制必须以“实际视觉位移”闭环，而不是输入事件数量。
- 一页的成功标准不是像素绝对精确，而是“有安全 overlap、无内容丢失、重复可去重”。

## 31. ScrollSession

```text
ScrollSession {
  window, region, direction, page_index, current_page, history, calibration
}
ScrollCalibration { pixels_per_wheel_step_ema?, preferred_overlap_ratio, max_batch }
PageSnapshot { scene_id, frame_id, region, content_signature, anchors[] }
ScrollAnchor { text?, bbox, patch_hash, uniqueness }
```

## 32. Anchor 选择

- 优先从当前页下方 60%H~90%H 选 2~4 个 anchor。
- 优先较长、当前页唯一、高置信度文本。
- 避免只用高重复时间戳、短词“我/好的”等。
- 联系人可用“名称 + 最近消息 preview + row patch hash”。
- 聊天可用较长消息 block + 邻近发言人/时间信息。
- 纯图片页可退化为 patch hash + neighboring text，并提高 overlap、减小滚动步长。

## 33. 滚动闭环算法

1. 对 current stable page 生成 PageSnapshot。
2. 选择 bottom anchors。
3. 计算 target_shift。
4. 根据 EMA 估算首个 wheel batch，batch 必须有上限。
5. 将鼠标移入 scroll region 安全点。
6. 注入小批量 wheel。
7. 等待滚动动画稳定。
8. 估算 actual_shift。
9. 更新 pixels_per_wheel_step_ema。
10. remaining = target_shift - accumulated_shift。
11. 若 remaining 仍大，继续小批量。
12. 接近目标时强制降到 1 wheel step。
13. 生成新 stable page snapshot。
14. 匹配 old bottom anchors 到 new top/middle。
15. 计算 overlap_ratio。
16. 若 overlap 可证明且位移合理 -> PageAccepted。
17. 若滚少 -> 继续小步。
18. 若滚过 -> PageOvershot，反向小步恢复。
19. overshoot recovery 有最大次数，例如 3。
20. 新页接受前绝不 append history。
21. 接受后用 overlap 去重 append history。
22. CurrentViewportScene 直接替换为新页 scene。

## 34. 实际位移估算

- 优先方法 A：相同 OCR anchor 在 old/new frame 的 y 位移。
- 如果 anchor 暂时丢失，方法 B：内容区中间窄 grayscale strip 做垂直 SAD/block matching。
- 方法 C：局部 patch hash / 小区域 SAD 辅助确认。
- 首版不需要 OpenCV；这些算法可以轻量 Rust 实现。
- 只有 metrics 证明需要 optical flow/feature matching 时再引入重型 CV 依赖。

## 35. Overshoot / undershoot

- 旧 anchors 全消失 + displacement 超目标太多 -> PageOvershot。
- Overshot 页不进入 history，先反向滚一个小步并重新稳定/匹配。
- 若 actual_shift 明显不足 -> 继续一个小 batch，不重复原大步。
- 控制器用 EMA 学习同一列表的实际 px/wheel step，越滚越准。
- 最大 recovery 次数耗尽 -> fail，宁可停止也不要跳过数据。

## 36. Page acceptance

- 新一页不能因为“滚动结束”就接受。
- 必须证明 old bottom anchor 在 new page 仍可找到。
- 最好有两类证据：text anchor + visual patch。
- overlap ratio 必须落在可接受范围，例如目标 15~25% 附近，最终范围需 benchmark。
- 若无法建立 overlap，返回“无法确认新旧页面连续”，停止采集。

## 37. History 去重与无旧数据污染

- 联系人 row signature：normalized text + relative geometry + optional avatar/icon patch hash。
- 新页 overlap 区匹配旧页末尾 rows，匹配部分只用于 continuity，不重复 append。
- 从第一个真正新 row 开始 append。
- 聊天使用 message block signature，不假设固定 row height。
- 当前 viewport 永远只来源于最新 stable page，不混入 history。

## 38. Scroll end detection

- 连续 2 次 wheel 后 actual_shift < min_shift。
- 同时 content_signature 基本不变。
- bottom anchors 原地不动。
- 可选使用 scrollbar/UIA ScrollPattern 作为附加证据。
- 满足组合条件 -> EndOfScroll；不要只因最后一行文字重复就判到底。

## 39. 联系人列表

- 先做 row clustering：OCR nodes 按 y band + 重复垂直节奏聚成 rows。
- row 内可有 name / preview / time 多个 token。
- 滚动 anchors 优先最后 2~3 个可见 rows。
- 目标只是找某个群时，每页 OCR 后立即 query，命中就停止，不需要扫完整列表。
- 有 Ctrl+F 搜索时优先搜索，智能滚动用于搜索不可用、批量采集或浏览。

## 40. 聊天历史

- 消息高度可变，不使用固定 row height。
- P0 用 text line proximity、左右对齐、vertical gap、时间 separator 推断 message block。
- anchor 选择较长且唯一的消息 block，短词与时间仅做辅助。
- 滚动期间若收到新消息导致 viewport 被强制拉到底，应检测 unexpected major shift 并 fail/recover。
- 实时内容顺序发生变化时，Strict crawl 应标记 `ScrollContentMutated`，不要拼接两套顺序。

## 41. Popup / Overlay 统一处理

- 独立 HWND popup：捕获为另一个 VisualSurface。
- 同 surface overlay：Dirty Region 检测出局部变化。
- 上层 VisualScene 统一查询，不要求先物理合成为一张超大 bitmap。
- popup 消失时相关 node/region 必须从 current scene 清除。

## 42. Evidence 集成

- 复用现有 `PreparedDiagnostics / EvidenceBundle / EvidenceRetentionPolicy`。
- 建议 Vision evidence：planner_explain.json、execution_context.json、window_topology.json、ocr_regions.json、visual_scene.json、compact_text.txt、spatial_text.txt。
- 允许时再保存 frame.png、dirty_map.png、ocr_overlay.png、scroll_state.json。
- `ocr_overlay` 可画 ROI、bbox、node id、confidence、region、anchors。
- 失败 evidence 记录 paddleocr_version、model、model_digest、engine、worker_build。
- Evidence capture 失败永远不能替代原始 AutomationError。

## 43. 隐私 / 数据边界

- 默认只捕获 AppSession WindowSet，不捕获整个桌面。
- raw frame 只放短期内存 ring buffer。
- 成功动作默认不持久化截图。
- OCR 文本同样视为敏感用户数据，遵守 retention policy。
- 密码/受保护 region 永不持久化明文。
- Evidence bundle 有 max bytes / TTL；4K raw BGRA 不落盘。
- worker 默认本地推理，不把屏幕发送到云服务。

## 44. 性能与调度指标

- 必须记录 capture_fps。
- 记录 stable_frame_latency。
- 记录 changed_area_ratio。
- 记录 ocr_roi_count 与 ocr_processed_pixels。
- 记录 tiny/medium OCR latency。
- 记录 scene merge/query latency。
- 记录 scroll settle latency。
- 记录 worker queue depth。
- 记录 cancelled stale OCR jobs。
- 关键比率：`ocr_processed_pixels / captured_pixels`。
- 没有 metrics 前不做 GPU compute diff 等过早优化。

## 45. GPU/CPU 数据路径

- 理想路径：D3D frame -> staging/readback 选定 ROI -> shared memory -> Python numpy。
- 首版若实现复杂，可先 full-frame CPU copy + ROI crop，但接口应允许 P1 做 ROI readback。
- 不要每帧 encode PNG 再交 worker；PNG 只适合 Evidence/debug。
- FramePool / shared memory ring 避免高频 allocation。
- 如果 OCR 跟不上 capture，不排队所有帧，只保留最新相关 generation。

## 46. crate / module 边界

```text
argusflow-windows:
  HWND, WGC, DXGI, D3D, DPI, topology, SendInput mouse/wheel
argusflow-vision:
  frame DTO, diff, stability, worker client, OCR merge, scene, layout,
  projections, query, cache, scroll, verification, evidence, metrics
src-tauri/runtime composition:
  construct Windows implementations and inject into VisionRuntime
```

- 不要让 `argusflow-vision` 到处直接调用 User32。
- 可以让 `WindowFrameSource` trait 定义在 vision，由 windows crate 实现，composition root 注入，避免循环依赖。

## 47. 推荐目录

```text
crates/argusflow-vision/src/
  backend.rs runtime.rs frame.rs diff.rs stability.rs
  worker/{client,protocol,health}.rs
  ocr/{result,merge}.rs
  scene/{node,region,cache}.rs
  layout/{lines,rows,regions}.rs
  projection/{compact,spatial}.rs
  query.rs
  scroll/{session,anchor,displacement,history}.rs
  verify.rs evidence.rs metrics.rs error.rs

crates/argusflow-windows/src/capture/
  wgc.rs dxgi.rs topology.rs frame_pool.rs mapping.rs dpi.rs error.rs

workers/argusflow-vision-worker/
  pyproject.toml requirements.lock src/argusflow_vision_worker/...
```

## 48. VisionRuntime 草案

```text
pub struct VisionRuntime {
  capture: Arc<dyn WindowFrameSource>,
  worker: Arc<VisionWorkerClient>,
  cache: Arc<VisualSceneCache>,
  topology: Arc<WindowTopologyTracker>,
  metrics: Arc<VisionMetrics>,
}
```

- VisualCacheBackend / OcrTinyBackend / OcrMediumBackend 共享同一个 Arc<VisionRuntime>。
- 不要每个 backend 自己开 capture stream / Python worker。

## 49. 关键 Rust 接口草案

```text
#[async_trait]
pub trait WindowFrameSource: Send + Sync {
  async fn open(&self, window: WindowIdentity, policy: CapturePolicy)
    -> Result<Arc<dyn FrameSubscription>, VisionError>;
}

#[async_trait]
pub trait OcrEngine: Send + Sync {
  async fn recognize(&self, request: OcrRequest)
    -> Result<OcrResponse, VisionError>;
}

#[async_trait]
pub trait VisualSceneService: Send + Sync {
  async fn current_scene(&self, window: WindowIdentity, policy: SceneRefreshPolicy)
    -> Result<Arc<VisualScene>, VisionError>;
}

pub fn evaluate_visual_query(scene: &VisualScene, query: &VisualQuery)
  -> Result<VisualMatch, AutomationError>;
```

## 50. VisionError 内部分类

- CaptureUnavailable。
- WindowIdentityChanged。
- FrameTimeout。
- FrameUnstable。
- WorkerUnavailable。
- OcrFailed。
- OcrCancelled。
- SceneStale。
- AmbiguousVisualTarget。
- ScrollOvershot。
- ScrollNoMovement。
- ScrollContentMutated。
- VerificationRejected。
- VerificationUncertain。

## 51. 配置建议：仅作为 benchmark seed

```text
[vision.diff]
scale = 0.25
tile_px = 32
roi_padding_px = 16
full_refresh_dirty_ratio = 0.35

[vision.stability]
min_frames = 2
timeout_ms = 800

[vision.scroll]
overlap_ratio = 0.18
max_recovery_attempts = 3
```

- 这些参数属于 runtime profile，不应保存到每个 workflow node。
- 产品层只需要“自动 / 快速 / 平衡 / 高精度”等 preset。

## 52. 产品设置建议

- 视觉识别：自动（UIA 不可用/需要验证时启用）。
- 识别强度：快速 = cache+tiny；平衡 = tiny+medium fallback；高精度 = 关键验证直接 medium。
- 发送前确认目标：默认开启。
- 发送后确认结果：默认开启。
- 开发者 Inspector：Visual Overlay / Compact Text / Spatial Text / Scene JSON。
- 普通用户不暴露 detection threshold、tile size 等模型细节。

## 53. 分阶段落地

### P0 — 看得见

- 实现 WGC by AppSession HWND
- 实现 topology tracker
- 实现 stable frame
- 接 PaddleOCR worker
- full OCR
- VisualScene
- Compact/Spatial Text
- Evidence

### P1 — 看得准

- diff ROI
- VisualCache
- OcrTiny/OcrMedium backend
- query 0/1/N
- search/header/send verification
- stale OCR cancel

### P2 — 点得准

- SendInput mouse
- bbox revalidation
- row safe rect
- click post-condition
- DPI/multi-monitor tests

### P3 — 滚得准

- wheel
- ScrollSession
- anchor
- displacement
- adaptive controller
- overshoot rollback
- history dedupe
- end detection

### P4 — 产品化

- shared-memory ring
- ROI GPU readback
- worker packaging
- offline model bundle
- metrics dashboard
- Vision Inspector
- benchmark

## 54. 推荐 PR 拆分

1. PR1：capture contract + WGC。
2. PR2：WindowTopologyTracker + DPI/coordinate。
3. PR3：Python worker + PaddleOCR 3.7.0 + health。
4. PR4：OCR protocol + shared/inline transport。
5. PR5：VisualScene + line clustering。
6. PR6：Compact/Spatial Text。
7. PR7：Diff + Dirty ROI + cache invalidation。
8. PR8：VisualCache/OcrTiny/OcrMedium backend + planner wiring。
9. PR9：微信 search/header/send verification。
10. PR10：SendInput mouse/wheel。
11. PR11：ScrollSession + anchor/displacement/controller。
12. PR12：History stitch + scroll E2E。
13. PR13：Evidence/Inspector/metrics polish。
14. PR14：packaging/offline/benchmark。

## 55. 第一优先级真实 Demo

- 连接微信 AppSession。
- Ctrl+F。
- Vision 确认 search state。
- 输入测试群名。
- OCR 搜索结果并输出 Compact/Spatial Text。
- exact 唯一确认目标群。
- Enter。
- OCR Header 并 exact 确认。
- 输入 nonce 消息。
- 记录 before_send_scene。
- Enter。
- Medium OCR bottom chat region。
- 确认新 outbound message。
- 打开 Evidence 查看 scene/ROI/model/version。

## 56. 第二 Demo：智能联系人分页

- 当前联系人 region 建 page snapshot。
- 输出当前 Compact/Spatial Text。
- 选择 bottom rows 作为 anchors。
- adaptive wheel 到约 82%H 内容位移。
- 匹配 overlap。
- 接受新页。
- CurrentViewportScene 替换。
- History 去重 append。
- 重复到 EndOfScroll。
- Test App 100 rows 要求 0 missing / 0 duplicate。

## 57. Test App：不要把 CI 绑死微信

- 建议做 ArgusFlow 自己的 Opaque Canvas Test App。
- 它只对 UIA 暴露一个大 Pane，内部 100 个自绘 rows。
- 包含 popup search、dynamic text、caret、variable-height chat blocks、resize/DPI。
- 这样 CI 可以稳定验证“自绘 UI 视觉底座”，微信只做可选人工/专用 E2E。

## 58. 必测矩阵

### Capture

- [ ] target HWND only
- [ ] 旁边另一个应用不进入 frame
- [ ] resize
- [ ] restore from minimized
- [ ] popup new HWND
- [ ] HWND/PID mismatch
- [ ] black-frame recovery

### DPI

- [ ] 100%
- [ ] 125%
- [ ] 150%
- [ ] secondary monitor negative origin

### Diff

- [ ] single row change
- [ ] large transition
- [ ] caret blink
- [ ] hover
- [ ] scroll animation
- [ ] ROI padding
- [ ] ROI count cap

### OCR

- [ ] Chinese
- [ ] mixed Chinese/English
- [ ] small font upscale
- [ ] tiny low confidence -> medium
- [ ] stale result cancel

### Scene

- [ ] ROI invalidation
- [ ] clean region preserved
- [ ] major transition full rebuild
- [ ] topology generation change
- [ ] semantic delta

### Projection

- [ ] tabs
- [ ] spaces
- [ ] multiline
- [ ] multi-column
- [ ] Unicode width
- [ ] deterministic output

### Query

- [ ] 0 candidate
- [ ] 1 candidate
- [ ] N candidates
- [ ] exact group
- [ ] fuzzy near miss only
- [ ] region filter

### Verification

- [ ] wrong search result
- [ ] wrong header
- [ ] send confirmed
- [ ] send uncertain no retry
- [ ] historical same message

### Scroll

- [ ] 100 fixed rows
- [ ] variable rows
- [ ] overshoot 1.4 page
- [ ] undershoot
- [ ] no movement end
- [ ] content mutation
- [ ] no stale current scene

### Evidence

- [ ] screenshots off
- [ ] screenshots on
- [ ] text redaction
- [ ] budget exceeded
- [ ] worker crash evidence

## 59. 智能滚动硬验收

- Test App 100 rows 扫描结果恰好 100 个 unique row。
- 不得缺号。
- 不得重复 append。
- 相邻已接受页必须至少有一个强 anchor overlap，推荐两个独立证据。
- 强制制造 overshoot 后必须回滚并恢复连续性。
- CurrentViewportScene 在每页接受后不得出现上页已离屏 row。
- Window resize / DPI change 必须使 ScrollSession 失效并重新建 continuity。
- 内容排序实时变化时 Strict 模式必须停止而不是静默拼接。

## 60. 微信安全硬验收

- 未确认搜索结果 exact 唯一时不得 Enter/点击。
- 进入聊天后 Header 不匹配时不得输入消息。
- 发送前记录 before_send scene generation。
- 发送后仅接受“新出现”的目标文本，不接受历史同文。
- 发送后验证 timeout 时不得自动再次 Enter。
- 用户切走前台导致 SendInput target mismatch 时必须停止。
- 任何 required verification 失败都产生可读 Evidence。

## 61. 性能验收方法

- 不要先承诺固定毫秒；先建立 benchmark corpus。
- 比较 full-frame OCR 与 dirty ROI OCR 的 processed pixel ratio。
- 比较 tiny-only、tiny->medium fallback、medium-only 的 P50/P95 延迟。
- 记录稳定窗口 idle 期间 OCR request 应接近 0。
- 记录滚动中间帧被 StableFrameGate 拦截的比例。
- 记录 stale OCR cancellation 数，验证 backpressure 生效。
- 只有指标证明瓶颈在 CPU diff/拷贝后再做 GPU diff/ROI readback。

## 62. 关键风险与对应策略

- **小字体 OCR**：ROI upscale + medium fallback + 高 DPI fixture。
- **动画导致永远 dirty**：StableFrameGate + temporal noise mask。
- **Popup/窗口重建**：dynamic topology generation + scene invalidation。
- **Python/模型依赖冲突**：独立 worker + pinned environment + health/warmup。
- **OCR 结果晚到污染 scene**：frame/generation check + cancellation。
- **误进群后误发**：search result + header 双重 exact verification。
- **发送重复**：NewTextExistsSince + uncertain no auto retry。
- **滚动跳页**：measured displacement + overlap acceptance + rollback。
- **滚动旧数据残留**：CurrentViewportScene / History 分离。
- **OCR 其他应用**：AppSession WindowSet scope + current-user IPC ACL。
- **Vision 黑盒化**：structured scene + 0/1/N + model provenance + Evidence。
- **微信更新**：generic layout first + profile only hints + no absolute coords。

## 63. 明确不做

- 不重写 AQL。
- 不把所有 UIA/CDP 流程降级成 OCR。
- 不默认全桌面截图。
- 不把 PaddleOCR-VL 当常规 GUI OCR。
- 不把 grounding 当默认 click engine。
- 不按 fuzzy highest score 偷偷选目标。
- 不按固定 wheel delta 宣称一页。
- 不让 Python worker 直接 SendInput。
- 不把窗口 class name 当稳定协议。
- 不把 screenshots/用户聊天内容无界持久化。
- 不在 Tauri command 里复制业务自动化。
- 不在 post-condition 不确定时自动重复非幂等动作。

## 64. 实施任务清单

- [ ] `CAP-001` `P0` WindowFrameSource trait
- [ ] `CAP-002` `P0` WGC HWND capture
- [ ] `CAP-003` `P0` WindowTopologyTracker
- [ ] `CAP-004` `P1` DXGI fallback
- [ ] `CAP-005` `P0` DPI/coordinate transform
- [ ] `CAP-006` `P0` capture health/black frame
- [ ] `FRM-001` `P0` FramePool
- [ ] `FRM-002` `P0` gray/downscale diff
- [ ] `FRM-003` `P0` DirtyRegion merge/padding
- [ ] `FRM-004` `P1` TemporalNoiseMask
- [ ] `FRM-005` `P0` StableFrameGate
- [ ] `WRK-001` `P0` Python worker project
- [ ] `WRK-002` `P0` pin PaddleOCR 3.7.0
- [ ] `WRK-003` `P0` PP-OCRv6 tiny
- [ ] `WRK-004` `P0` PP-OCRv6 medium
- [ ] `WRK-005` `P0` Named Pipe protocol
- [ ] `WRK-006` `P1` shared memory transport
- [ ] `WRK-007` `P0` worker supervisor
- [ ] `WRK-008` `P0` OCR cancellation/backpressure
- [ ] `SCN-001` `P0` VisualScene/VisualNode
- [ ] `SCN-002` `P0` OCR merge
- [ ] `SCN-003` `P0` ROI invalidation
- [ ] `SCN-004` `P0` line clustering
- [ ] `SCN-005` `P1` row clustering
- [ ] `SCN-006` `P1` region inference
- [ ] `TXT-001` `P0` CompactTextProjection
- [ ] `TXT-002` `P0` SpatialTextProjection
- [ ] `QRY-001` `P0` VisualQuery exact 0/1/N
- [ ] `QRY-002` `P1` explicit fuzzy candidates
- [ ] `BE-001` `P0` VisualCacheBackend
- [ ] `BE-002` `P0` OcrTinyBackend
- [ ] `BE-003` `P0` OcrMediumBackend
- [ ] `EVD-001` `P0` Vision PreparedDiagnostics
- [ ] `EVD-002` `P0` retention policy integration
- [ ] `INP-001` `P1` mouse click
- [ ] `INP-002` `P1` wheel
- [ ] `SCR-001` `P1` ScrollSession
- [ ] `SCR-002` `P1` anchor selection
- [ ] `SCR-003` `P1` displacement estimation
- [ ] `SCR-004` `P1` adaptive wheel controller
- [ ] `SCR-005` `P1` page acceptance
- [ ] `SCR-006` `P1` overshoot rollback
- [ ] `SCR-007` `P1` history dedupe
- [ ] `SCR-008` `P1` end detection
- [ ] `WX-001` `P0` search state verify
- [ ] `WX-002` `P0` group result exact verify
- [ ] `WX-003` `P0` chat header exact verify
- [ ] `WX-004` `P0` send post-condition verify
- [ ] `WX-005` `P1` contact smart paging
- [ ] `WX-006` `P1` chat history paging
- [ ] `TST-001` `P0` Opaque Canvas Test App
- [ ] `TST-002` `P0` capture E2E
- [ ] `TST-003` `P0` diff golden tests
- [ ] `TST-004` `P0` OCR golden tests
- [ ] `TST-005` `P1` 100-row scroll E2E
- [ ] `MET-001` `P0` vision metrics
- [ ] `UI-001` `P1` Vision Inspector
- [ ] `PKG-001` `P2` worker/model packaging

## 65. 每个任务的 Definition of Done

- [ ] 不绕过 AppSession / AutomationExecutionScope。
- [ ] HWND/PID 关键路径有复验。
- [ ] 有 timeout/deadline。
- [ ] 有 stale generation 处理。
- [ ] 有结构化 error。
- [ ] 有 Planner Explain 或 runtime trace。
- [ ] 涉及 OCR 时保留 model/version provenance。
- [ ] 涉及像素/文本时遵守 EvidenceRetentionPolicy。
- [ ] 有 unit/golden test。
- [ ] Windows-specific 行为有 integration test 或明确验收步骤。
- [ ] 不会在失败时 silently choose first/highest score。
- [ ] 不会让旧 viewport node 污染 current scene。
- [ ] 不会破坏已有 UIA/CDP 行为。
- [ ] 有相应 metric 或可观察日志。

## 66. Code Review 自检问题

1. 这个 OCR result 属于哪个 frame_id？
2. generation 过期时谁阻止它写 scene？
3. bbox 属于哪套坐标？
4. 点击前是否重新验证 HWND/PID？
5. popup 独立 HWND 与同 surface overlay 都能工作吗？
6. OCR 是否真的只处理必要 ROI？
7. dirty ROI 是否有 padding？
8. caret 会不会造成永久 OCR？
9. 什么时候 full refresh？
10. CurrentViewportScene 与 History 是否完全分开？
11. Page 是否在找到 overlap 前就 append 了？
12. overshoot 能检测并回滚吗？
13. 是否把 wheel step 错当 pixel？
14. 内容重排时是否会错误拼接？
15. tiny 低置信度是否升级 medium？
16. medium 是否被滥用全帧？
17. fuzzy 是否只产生候选？
18. Evidence 是否记录 model/version？
19. screenshots off 时是否仍有 scene/boxes 诊断？
20. OCR text 是否按敏感数据处理？
21. worker crash 是否拖死 Workflow？
22. worker restart 是否有上限？
23. 是否混装冲突 inference engine？
24. 是否存在固定微信绝对坐标？
25. 是否把 window class 当唯一协议？
26. 是否复用 PreparedPlan target wait？
27. 发送验证失败是否错误自动重发？
28. UIA 可用目标是否仍能优先 UIA？
29. metrics 能否证明 ROI 优化有效？

## 67. 推荐视觉状态机：微信

```text
Idle -> AppReady -> SceneReady
-> SearchOpening -> SearchReady -> SearchResultsReady
-> SearchTargetConfirmed -> ChatOpening -> ChatHeaderConfirmed
-> DraftReady -> Sending -> SendPendingVerification -> Completed
任何 Required verification 失败 -> Failed / Uncertain
```

- 状态携带 app_session_id、WindowIdentity、scene_id、topology_generation。
- 进入新阶段前刷新 scene id，避免拿旧阶段 node 执行新阶段动作。

## 68. 推荐日志文案

- 正在确认微信搜索结果…
- 已确认目标群聊：项目讨论群。
- 正在确认已进入目标群聊…
- 顶部标题未确认，为避免误发消息已停止。
- 正在确认消息发送结果…
- 已执行发送键，但未在验证时间内确认新消息；为避免重复发送，不会自动再次发送。
- 列表已滚动，但无法确认新旧页面可靠重叠；为避免跳过条目，采集已停止。

## 69. 最优先的五件事

1. WGC by AppSession HWND。
2. PaddleOCR 3.7.0 worker。
3. VisualScene + Compact/Spatial Text。
4. Dirty ROI + no-stale merge。
5. 微信 search/header/send verification。

## 70. 为什么智能滚动排在视觉验证之后

- 当前微信键盘流程已经能完成搜索、输入、回车，真正危险的是“是否进对群”和“是否真的发成功”。
- 先建立观察/验证闭环，可以在不实现视觉鼠标的情况下立刻显著提升可靠性。
- 滚动依赖已有 StableFrame、VisualScene、Diff、Input wheel，因此自然是后续能力。

## 71. 最终架构图

```text
Workflow / AppSession
      |
ActionRouter / PreparedPlan
      |---------------- UIA / CDP
      |
      +-> VisionRuntime
            |-> WGC WindowSet
            |-> StableFrame + Diff
            |-> PaddleOCR Worker
            |-> VisualScene
            |     |-> CompactText
            |     |-> SpatialText
            |     |-> Query / Verification
            |
            +-> Scroll feedback
                     |
                  SendInput
                     |
                 Verify again
```

## 72. 最终滚动图

```text
Current Stable Page
  -> choose bottom anchors
  -> target shift = H * (1-overlap)
  -> small wheel batch
  -> wait stable
  -> measure actual shift
  -> more / rollback
  -> match old/new anchors
  -> accept only with overlap
  -> replace CurrentViewportScene
  -> append deduped History
```

## 73. 参考仓库文件

- `docs/ArgusFlow_真实_UIA_对接方案_NotepadPP_E2E.md`
- `docs/ArgusFlow_Selector_Resilience_Failure_Evidence_Design.md`
- `docs/ArgusFlow_节点内建等待与UI就绪同步方案.md`
- `docs/ArgusFlow_App_Run_Node_Design.md`
- `crates/argusflow-vision/src/lib.rs`
- `crates/argusflow-windows/src/capture/mod.rs`
- `crates/argusflow-agent/src/context.rs`
- `crates/argusflow-agent/src/evidence.rs`
- `crates/argusflow-windows/src/window/application.rs`
- `crates/argusflow-windows/src/input/backend.rs`
- `crates/argusflow-core/src/automation.rs`

## 74. PaddleOCR 官方资料

- 官方仓库：`https://github.com/PaddlePaddle/PaddleOCR`。
- PP-OCRv6：`https://www.paddleocr.ai/latest/en/version3.x/algorithm/PP-OCRv6/PP-OCRv6.html`。
- OCR pipeline：`https://www.paddleocr.ai/latest/en/version3.x/pipeline_usage/OCR.html`。
- 安装：`https://www.paddleocr.ai/latest/en/version3.x/installation.html`。
- Inference engine：`https://www.paddleocr.ai/latest/en/version3.x/inference_deployment/local_inference/inference_engine.html`。

## 75. 最终推荐

- 用 WGC 按 frozen AppSession HWND 捕获，不以全桌面作为默认输入。
- 用 WindowTopologyTracker 补动态 Owned Popup / same-process windows。
- 用 StableFrameGate + low-res tile diff 只刷新必要 ROI。
- 用 PaddleOCR 3.7.0 / PP-OCRv6 tiny 做高频增量，用 medium 做升级与关键验证。
- 用 VisualScene 保存结构化事实，用 CompactText/SpatialText 提供你想要的文本化“应用面貌”。
- 用 PreparedPlan 继续管理 target wait/fallback，不在 OCR backend 自造重试语义。
- 用 Visual verification 给当前微信键盘流补齐 search/header/send 三个关键安全断言。
- 用视觉位移 + anchors + overlap acceptance 定义“一页”，而不是固定滚轮量。
- 用 CurrentViewportScene / ScrollDocumentHistory 分离，彻底解决旧信息残留。
- 这样得到的是一套可复用于 Qt、Canvas、DirectComposition、游戏式 UI、自研 GPU UI 的通用 Windows Vision substrate，而不只是微信专用脚本。

## 76. 可直接转 Issue 的详细验收清单

### Capture

- [ ] WGC 只输出 target HWND 的内容
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] frame 带 hwnd/pid
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] frame resize 更新尺寸
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 窗口关闭返回结构化错误
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 恢复后能重建 subscription
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 黑帧可检测
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 捕获健康状态进入 Planner availability
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 不默认落盘
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 同 PID popup 可加入 WindowSet
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] popup 消失会失效 scene
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] DXGI fallback 不改变坐标契约
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 多显示器负坐标可转换
  - 测试：`TBD`
  - 指标/Evidence：`TBD`

### Diff/Stability

- [ ] idle 不触发 OCR
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 单行变化只 dirty 局部
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] ROI 有文字 padding
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 大变换自动 full refresh
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] caret 被 noise mask
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 真实输入逃逸 noise mask
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 滚动中间帧不 publish stable scene
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 稳定 deadline 生效
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] ROI 数过多自动合并
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] changed_area_ratio 可观测
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] dirty region 会 invalidates old node
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] clean region node 保留
  - 测试：`TBD`
  - 指标/Evidence：`TBD`

### Worker/OCR

- [ ] worker handshake 版本匹配
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] paddleocr_version == 3.7.0
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] tiny lazy load
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] medium lazy load
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 中文 warmup
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 英文 warmup
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] request deadline
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] request cancellation
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] queue bound
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] stale result drop
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] bbox polygon 返回
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] confidence 返回
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] model provenance 返回
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 小字体可 upscale
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] medium fallback 可重跑同 ROI
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] worker crash bounded restart
  - 测试：`TBD`
  - 指标/Evidence：`TBD`

### Scene

- [ ] scene_id 单调
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] frame_id 正确
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] topology_generation 正确
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] node bbox 坐标明确
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] raw_text 保留
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] normalized_text 可解释
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] source provenance
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] line clustering deterministic
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] row clustering deterministic
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] region inference deterministic
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] semantic delta 可查询
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] old generation 不可 current query
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] VisualCache freshness 可解释
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] full rebuild 清理旧 popup node
  - 测试：`TBD`
  - 指标/Evidence：`TBD`

### TextProjection

- [ ] CompactText 同行合并
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 大 gap 用 tab
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 普通 gap 用 space
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 跨行 newline
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] region marker 可开关
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 中文宽度处理
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] SpatialText 固定归一列数
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] SpatialText deterministic
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 投影不修改 raw OCR 文本
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 投影不是永久 selector
  - 测试：`TBD`
  - 指标/Evidence：`TBD`

### Query/Planner

- [ ] exact 0 -> TargetNotFound
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] exact 1 -> success
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] exact N -> AmbiguousTarget
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] fuzzy 不自动选第一
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] near miss 只进 trace/evidence
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] cache hit 不重复 OCR
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] dirty target region 强制 refresh
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] prepare 冻结 HWND/PID
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] execute revalidate identity
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] target miss 进入 PreparedPlan wait
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] OCR backend 不隐藏固定 sleep
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] worker not ready => availability not ready
  - 测试：`TBD`
  - 指标/Evidence：`TBD`

### Input/Verify

- [ ] 发送前 window foreground 检查
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] bbox stale 时不点击
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] dirty bbox 前重新 OCR
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] search state required verify
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] group result exact required verify
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] header exact required verify
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] before_send_scene 记录
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 新消息必须 scene delta added
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 历史同文不算新消息
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] postcondition timeout 不重发
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] Uncertain 有用户可读错误
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] Evidence 能看到 verifier 条件
  - 测试：`TBD`
  - 指标/Evidence：`TBD`

### Scroll

- [ ] ScrollSession 绑定 window/region
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 每页多个 anchor
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] anchor 优先唯一长文本
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 目标 shift 由 H/overlap 定义
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] wheel 小批次
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 实际 shift 可测
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] EMA 学习 px/step
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 接近目标降为 1 step
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] PageAccepted 前必须 overlap
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] overshoot 页不 append
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 反向 rollback 有上限
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] undershoot 继续小步
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] current scene 替换而非累加
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] history overlap dedupe
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 100 row 无缺失
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 100 row 无重复
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] variable-height chat 可连续
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] 无移动组合判 EndOfScroll
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] resize 使 session 失效
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] content mutation Strict fail
  - 测试：`TBD`
  - 指标/Evidence：`TBD`

### Evidence/Privacy

- [ ] screenshots default off
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] OCR text 按敏感数据
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] raw BGRA 不持久化
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] Evidence max bytes
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] Evidence TTL
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] password region redacted
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] worker/model/version 入 manifest
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] scene json 可 diff
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] ocr regions 有 ROI reason
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] overlay 可显示 anchor
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] evidence failure 不覆盖原 error
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
- [ ] WindowSet 不包含其他进程
  - 测试：`TBD`
  - 指标/Evidence：`TBD`
