# ArgusFlow 最近两次提交深度审计与整改方案（1500 行核心版）
> 仓库：`SLOE-debug/argusflow`
>
> 审计分支：`main`
>
> 审计日期：2026-08-28
>
> 最近提交：`e7d0169d98a3e8bd6ed0ef486095f95a5863a4c5` — `feat: 完成视觉定位链路与工作台重构`
>
> 前一提交：`7335750b62563f7b49cf605db626685ba5f177b0` — `feat: 落地微信视觉感知与 PaddleOCR 执行链路`
>
> 对照基线：`6ffa38ad8d1a8b9587044cc493add8b959ed2944`
>
> 审计性质：静态代码审计、跨 crate 契约审计、docs 一致性审计、风险与整改方案。
> 说明：任务背景称这两次提交由实习生完成；本文只审实现，不根据 GitHub 元数据推断个人身份或能力。
## 0. 结论先行
这两次提交不是“全部推倒重来”。
正确方向不少：`ValueExpr` 进入视觉查询、`NormalizedRect` 建立反序列化不变量、视觉查询坚持 0/1/N、Preset 不新增 Runtime type、默认流程删除固定 Delay、SendInput 做 HWND/PID/前台复验，这些都应该保留。
但当前 main 同时存在多个会直接破坏正确性或架构边界的问题。
- Vision `GetValue/Extract` 的实际输出 key 与 Runtime 声明的端口不一致。
- BGRA 像素被 `Vec<u8>` 通过 JSON 数字数组塞进 4MB Named Pipe frame，普通窗口 ROI 就有超帧风险。
- `DirtyMap` 目前主要用于 cache invalidation，并没有真正驱动下一次 OCR 的 refresh ROI。
- Visual Click 由 SendInput 内部隐藏调用 VisionRuntime，并默认走 medium + force refresh，绕过 Router 对 cache/tiny/medium 的统一选择。
- `VisualResolved` 注释称 runtime-only，却位于可 Serialize/Deserialize 的持久化 `TargetLocator` 枚举中。
- 发送消息后的 `exact(message)` 只证明屏幕上存在同文案，不能证明本次非幂等发送产生了新消息。
- docs 说 dynamic WindowSet / OwnedPopup，但现有 WGC 仍只 capture primary HWND，topology 更多只是 generation invalidation。
- VisionRuntime 有多 subscription，却只有单全局 scene cache / last frame / noise mask，窗口切换会互相清状态。
- 默认微信 Workflow 与官方 Flow Component 重复硬编码同一完整业务图，已经形成两份业务真相。
建议：暂停新增 Vision feature，先用 6～8 个小 PR 修主链。
如果当前仍在 PR review 阶段，我会对两个提交都给 `Request Changes`，但不建议整包 revert。
### 0.1 严重度
| 等级 | 判断 |
|---|---|
| P0 | 会直接破坏正确性、协议可用性、非幂等安全或核心架构边界 |
| P1 | 高概率导致性能数量级退化、状态错误、重复实现或维护失控 |
| P2 | 当前可运行，但扩展后容易漂移，或缺少必须的 integration proof |
### 0.2 最先修的六项
1. 锁住 Vision ActionOutcome 输出契约。
2. 替换 JSON 像素数组传输。
3. 让 DirtyMap 生成真正的 OCR RefreshPlan。
4. 把视觉 materialization 收回 Planner/PreparedPlan。
5. 移除 persisted `VisualResolved`。
6. 把微信 send verification 改成 before/after delta，并支持 Unknown outcome。
## 1. 审计依据与 docs 约束
重点参考 docs：
- `docs/架构.md`
- `docs/ArgusFlow_节点原子性与预制子流程设计方案.md`
- `docs/ArgusFlow_节点内建等待与UI就绪同步方案.md`
- `docs/ArgusFlow_微信视觉感知_PaddleOCR_v3.7_实施方案.md`
- `docs/ArgusFlow_微信OCR键鼠与Studio信息架构重构方案_7335750_1500行版.md`
本次把以下规则当强约束，而不是“以后再优化”的建议：
1. 不重造第二套 Runtime。
2. Primitive 表达稳定业务语义，不表达 Win32/OCR 低层步骤。
3. Preset 只预填 Primitive，不增加 Runtime type。
4. Component 承担业务级复用。
5. UIA/CDP 是语义快路径。
6. Vision 是像素事实层、fallback 与 verification。
7. SendInput 是物理 actuation。
8. readiness wait 属于当前节点/PreparedPlan。
9. Delay 只表示真实固定暂停。
10. 视觉目标必须严格 0/1/N。
11. fuzzy 不能把最高分偷偷升级成执行目标。
12. 非幂等动作必须明确 post-condition 与 retry 语义。
13. 动态输入必须沿用 ValueExpr。
14. OCR response 必须绑定 frame/generation。
15. Current viewport 与 scroll history 分离。
16. 默认不做全桌面 OCR。
17. visual scope 必须来自 AppSession/HWND/PID。
18. DPI/client-to-screen 归 Windows 层。
19. Backend fallback/cost/evidence 应由 Planner 统一控制。
20. 失败证据应能标出真实失败 stage/backend。
## 2. 两次提交的风险画像
### 2.1 `7335750`
- 相对 `6ffa38a` 约 63 个文件变化。
- 新增约一万行量级。
- 同一提交一次性建立 Vision crate、WGC、diff、stability、scene、OCR、worker、scroll、metrics、evidence、mouse input 与 Tauri wiring。
- scroll 子系统单独约新增 1346 行。
- 同提交还加入约 1555 行视觉方案文档。
- 这是典型 big-bang 基础设施提交：模块很多，但最基础的纵向 contract 没有先被 E2E 锁住。
### 2.2 `e7d0169`
- 相对 `7335750` 为 47 个文件变化。
- 新增约 3500 行量级，其中约 1560 行是新方案文档。
- 主要加入 VisualQueryExpr/NormalizedRect、Visual Click、微信默认视觉闭环、Preset/Component Studio 重构。
- 方向比上一提交聚焦，但继续建立在未修复的 transport/output/refresh 底座上。
### 2.3 CI 可见性
GitHub connector 对这两个 SHA 没有返回 PR-triggered workflow runs。
这不能绝对推出仓库没有任何 CI，但至少本次审计没有看到这两个 commit 的 PR CI 运行证据。
对这种跨 Rust/Python/Windows/React 的底层改造，缺少可见 integration signal 是高风险。
## 3. F01 [P0] VisionBackend 输出 key 与 Runtime 节点端口不一致
**归属提交**：7335750（e7d 未修）
**关键文件**：
- `crates/argusflow-vision/src/backend.rs`
- `crates/argusflow-runtime/src/builtin_nodes/ui.rs`
### 现状证据
- Runtime 明确声明 `GetText -> text`。
- Runtime 明确声明 `GetValue -> value`。
- Runtime 明确声明 `Extract One -> item`。
- Runtime 明确声明 `Extract Many -> items`。
- VisionBackend 把 `GetText | GetValue` 合并处理，并统一写入 `text`。
- VisionBackend 对 Extract 不区分 cardinality，统一写入 `value`。
### 影响
- GetValue 的下游 `ValueExpr` 按 `value` 取值时拿不到结果。
- Visual Extract 的下游按 `item/items` 取值时拿不到结果。
- 节点本身可能返回成功，错误会延迟到后续节点或 Component output，诊断更困难。
### 为什么这是问题
这是跨 crate 的公开 contract 破坏，不是内部命名风格问题。
### 整改建议
1. 把输出 key 的定义收敛为单一 `ActionOutputContract` 或 `AutomationAction::output_shape()`。
2. `GetText` 返回 `text`。
3. `Extract One` 返回 `item`，`Extract Many` 返回 `items`。
4. 不要让每个 Backend 手写 port string。
### 最低测试门槛
- [ ] 跨 backend contract test：Runtime 声明的 output descriptor 必须等于 ActionOutcome keys。
- [ ] Visual GetText / Extract One / Extract Many 回归测试。
---
## 4. F02 [P0] Vision GetValue 用 OCR 可见文本冒充控件 value
**归属提交**：7335750
**关键文件**：
- `crates/argusflow-vision/src/backend.rs`
### 现状证据
- `supports_observation` 把 `GetValue` 宣布为视觉可支持动作。
- `execute_observation` 对 GetValue 返回 OCR `raw_text`。
- OCR 只能证明可见文本，不能天然证明语义控件 value。
### 影响
- placeholder、密码框、slider、checkbox、语义 value pattern 都可能被错误解释。
- Planner 会认为 Vision 对 ReadValue 有真实语义能力，实际上只是 ReadText。
### 为什么这是问题
Backend capability 必须语义诚实；“能看到字”不等于“能读控件值”。
### 整改建议
1. P0 直接让 Vision GetValue 返回 Unsupported。
2. 未来只有在 VisualScene 有明确 value 事实来源时再开启。
3. 保留 Extract 的 Text/Name 支持，Value/Property/Attribute 继续 Unsupported。
### 最低测试门槛
- [ ] Visual GetValue 必须稳定返回 ActionUnsupported/Backend unsupported，而不是 text。
---
## 5. F03 [P0] BGRA `Vec<u8>` 被编码成 JSON 数字数组，4MB frame 对正常 ROI 不够
**归属提交**：7335750
**关键文件**：
- `crates/argusflow-vision/src/worker/protocol.rs`
- `crates/argusflow-vision/src/worker/client.rs`
- `workers/argusflow-vision-worker/src/argusflow_vision_worker/protocol.py`
- `workers/argusflow-vision-worker/src/argusflow_vision_worker/worker.py`
### 现状证据
- `PixelTransport::InlineBytes` 持有 `Vec<u8>`。
- 外层协议使用 `serde_json::to_vec` / Python `json.loads`。
- Rust 与 Python 都把控制 frame 最大值限制为 4MB。
- Python 端要求 `bytes` 是 `list[int]`，逐个校验，再 `numpy.asarray`。
- JSON 中每个 raw byte 会膨胀为 1～3 位十进制字符加分隔符。
- 默认微信 search/message ROI 约覆盖窗口 42%/48%。
### 影响
- 1200×800 这种普通窗口的大区域 raw BGRA 已有约 1.6～1.8MB，JSON 后极易超过 4MB。
- 更大窗口、高 DPI、全帧刷新会直接协议失败。
- 即使没超限，JSON parse + Python list[int] 也造成显著 CPU/内存浪费。
### 为什么这是问题
像素平面是二进制数据，不应进入 JSON number array；这已是 correctness 问题而不只是性能问题。
### 整改建议
1. P0 改为 JSON header + raw binary body，或直接实现已有 SharedMemory variant。
2. Python 用 `memoryview` / `numpy.frombuffer`，不要构造 `list[int]`。
3. 保持控制面 frame 上限和独立 pixel body 上限。
4. 不要用单纯把 4MB 改成 64MB 的方式掩盖。
### 最低测试门槛
- [ ] 256×256、800×600、1200×800 50% ROI、高熵像素、stride padding、截断 body、错误 token。
---
## 6. F04 [P0/P1] DirtyMap 只驱动 cache invalidation，没有真正驱动 OCR refresh ROI
**归属提交**：7335750
**关键文件**：
- `crates/argusflow-vision/src/runtime.rs`
- `crates/argusflow-vision/src/scene/cache.rs`
### 现状证据
- `update_cache_invalidation` 会计算 DirtyMap 并 invalidate cache。
- 之后 OCR ROI 实际仍是 `query_region.unwrap_or_else(frame.bounds)`。
- 没有从 DirtyMap 生成待识别 regions。
- 没有把 query region 与 dirty regions 做交集。
### 影响
- docs 承诺的“只重新识别变化 ROI”没有完整落地。
- cache miss 或 force refresh 时很容易继续整帧/整区域 OCR。
- 进一步放大 F03 的 IPC 大小与 OCR latency。
### 为什么这是问题
Diff 目前只实现了“哪些旧事实不可复用”，没实现“哪些像素需要重新识别”。
### 整改建议
1. 新增 `RefreshPlan { CacheOnly | Partial(regions) | Full }`。
2. DirtyMap -> merge/pad -> coverage -> RefreshPlan。
3. query region 与 dirty region 做交集而非二选一。
4. major transition/topology change/无 base scene 才 Full。
5. 记录 `ocr_pixels/captured_pixels` 证明增量有效。
### 最低测试门槛
- [ ] 无变化 CacheOnly、小 dirty Partial、大 coverage Full、query∩dirty、stale response。
---
## 7. F05 [P0] Visual Click 在 SendInput 内隐藏调用 VisionRuntime，绕过 Router
**归属提交**：e7d0169
**关键文件**：
- `crates/argusflow-windows/src/input/backend.rs`
- `crates/argusflow-windows/src/input/visual_resolver.rs`
- `crates/argusflow-agent/src/visual.rs`
### 现状证据
- Router 对 Click(Visual) 最终选择的是 SendInput candidate。
- VisionBackend 明确不支持 Click。
- SendInput execution 再调用 `VisualTargetResolver::resolve`。
- 默认 VisualResolvePolicy 为 `prefer_medium=true`、`force_refresh=true`。
- Windows resolver 因而直接调用 `SceneRefreshPolicy::medium()`。
### 影响
- Router 看不到真实的 VisualCache/Tiny/Medium stage。
- Explain 的 SendInput Low cost 与真实 medium OCR 成本不一致。
- cache hit 和 tiny success 无法按统一 planner 规则优先。
- fallback/evidence/error attribution 被隐藏在 SendInput 里面。
### 为什么这是问题
SendInput 应是 actuator，不应再成为一个局部视觉 Router。
### 整改建议
1. 引入 planner-owned `PreparedTargetMaterializer`。
2. 视觉 materialization 顺序由 PreparedPlan 控制：Cache -> Tiny -> Medium -> Grounding。
3. materialize 产出带 window/frame/scene/generation 的 `MaterializedTarget`。
4. SendInput 只复验窗口/点位并注入。
### 最低测试门槛
- [ ] cache unique 不 OCR、tiny unique 不 medium、medium fallback、ambiguous 不点击、Explain 包含 materializer 与 actuator。
---
## 8. F06 [P0] runtime-only `VisualResolved` 被放进可 serde 的 persisted TargetLocator
**归属提交**：e7d0169
**关键文件**：
- `crates/argusflow-core/src/automation.rs`
- `crates/argusflow-runtime/src/builtin_nodes/ui.rs`
### 现状证据
- `TargetLocator` derive `Serialize, Deserialize`。
- 同一 enum 新增 `VisualResolved { query: VisualQuery }`。
- 注释明确写“只存在一次动作执行的内存契约，不应由编辑器持久化”。
- 但类型系统没有阻止它被序列化/反序列化。
### 影响
- runtime state 可能泄漏进 workflow JSON、fixture、evidence 或未来编辑器保存路径。
- persisted schema 与 prepared state 生命周期混在一起。
### 为什么这是问题
这是用注释维护不可持久化不变量，与项目强类型边界原则冲突。
### 整改建议
1. `TargetLocator` 只保留 persisted semantic locator。
2. 新增内部 `PreparedTargetLocator` / `ResolvedAutomationTarget`。
3. `VisualQueryExpr` 在 prepare 时解析成 `VisualQuery`，后续不再回写 persisted enum。
### 最低测试门槛
- [ ] workflow JSON 不允许 `visual_resolved`、旧 string visual query 可迁移、prepared type 不实现 Serialize。
---
## 9. F07 [P0] 微信发送后的 exact-text 验证可把历史同文本误判成本次发送成功
**归属提交**：e7d0169
**关键文件**：
- `src/features/workflow/model/defaultWorkflowTemplate.ts`
- `src/features/workflow/components/componentCatalog.ts`
### 现状证据
- 当前流程是 `TypeText -> Enter -> Visual GetText exact(message)`。
- 验证只要求当前 message region 存在相同文本。
- 没有记录发送前 scene 或匹配集合。
- 没有要求 post scene 中出现“新增”节点/row。
### 影响
- 历史里已有同文本时，本次 Enter 即使失败也可能判成功。
- 非幂等动作的 false success 比普通 target miss 更危险。
### 为什么这是问题
post-condition 必须证明新事实，而不是证明一个可能早已存在的事实。
### 整改建议
1. 发送前记录 `PreSendObservation`：scene/frame、matching hashes、bottom anchor。
2. 只发送一次。
3. 发送后强制获取新 stable scene。
4. 要求新 matching node/row 不属于 pre-send visible set，并符合 appended position。
5. 无法证明时返回 `OutcomeUnknown`，禁止自动重发。
### 最低测试门槛
- [ ] 历史同文+本次失败不能成功、本次新增成功、post scene 不可用 -> Unknown、Unknown 不重试。
---
## 10. F08 [P1] dynamic WindowSet/OwnedPopup 目前主要只是 generation invalidation，并非真实多 surface capture
**归属提交**：7335750
**关键文件**：
- `crates/argusflow-windows/src/capture/topology.rs`
- `crates/argusflow-windows/src/capture/wgc.rs`
- `crates/argusflow-vision/src/runtime.rs`
### 现状证据
- TopologyTracker 枚举 Primary/OwnedPopup/SameProcessTopLevel。
- WGC open 仍只为 primary HWND 创建一个 GraphicsCaptureItem。
- topology refresh 的主要结果是给 frame stamp generation。
- VisionRuntime 当前 scene 仍绑定单一 WindowIdentity/frame。
### 影响
- popup 出现会让 cache 失效，但 popup pixel 不一定进入 primary frame。
- docs 中 dynamic WindowSet 的能力承诺高于真实实现。
### 为什么这是问题
“知道 popup 存在”和“能观察 popup 像素”是两件不同的事。
### 整改建议
1. P0 若只支持 primary surface，就明确写限制。
2. 若要支持，建立 `VisualSurfaceSet`，分别 capture allowed same-session surfaces。
3. 每个 VisualNode 带 surface identity，并在全 surface set 上做唯一性。
### 最低测试门槛
- [ ] 真实 owned popup、同 PID 非 owned window、其它进程 popup、跨 surface ambiguity。
---
## 11. F09 [P1] VisionRuntime 是多 subscription + 单全局 cache/frame/noise 状态
**归属提交**：7335750
**关键文件**：
- `crates/argusflow-vision/src/runtime.rs`
- `crates/argusflow-vision/src/scene/cache.rs`
### 现状证据
- `subscriptions` 可以存多个 WindowIdentity。
- `cache` 只有一个 VisualSceneCache。
- `last_stable_frame` 只有一个 `(window, frame)`。
- `temporal_noise` 只有一个 mask。
- 切换 window 时会 clear cache/noise。
### 影响
- A->B->A 会不断互相清场并重新 OCR。
- 并行 workflow、多 AppSession、未来 popup surface 都会产生结构性 thrash。
### 为什么这是问题
状态 key 与公开支持的 window scope 数量不一致。
### 整改建议
1. 改成 `HashMap<VisualScopeKey, ScopeState>`。
2. ScopeState 内含 subscription/cache/last_frame/noise/freshness/metrics。
3. 加 idle TTL/LRU/prune，关闭失效 subscription。
### 最低测试门槛
- [ ] A/B/A cache isolation、多 session 并发、关闭重开、LRU eviction。
---
## 12. F10 [P1] 默认微信 Workflow 与官方发送微信群消息 Component 重复硬编码同一业务图
**归属提交**：e7d0169
**关键文件**：
- `src/features/workflow/model/defaultWorkflowTemplate.ts`
- `src/features/workflow/components/componentCatalog.ts`
- `src/features/workflow/model/wechatTemplateParts.ts`
### 现状证据
- 两处都硬编码 Acquire WeChat、Ctrl+F、搜索验证、输入群名、找群、点击、header 验证、输入消息、Enter、消息验证。
- ApplicationSpec 也重复定义。
- 虽然 helper 被抽到 wechatTemplateParts，但 graph topology 仍是两份。
### 影响
- 任何 node version、wait、region、verification、output 改动都可能只改一份。
- 默认模板与官方 Component 会产生行为漂移。
### 为什么这是问题
docs 已明确“发送微信群消息”应是 Flow Component；默认模板不应再维护第二份 canonical graph。
### 整改建议
1. 把 `createWechatMessageDefinition()` 作为唯一 graph source。
2. 默认 workflow 直接实例化 Component，或从 canonical definition 展开展示。
3. ApplicationSpec 也收敛到单一 helper。
### 最低测试门槛
- [ ] 默认模板展开图与 component definition hash/semantic snapshot 一致。
---
## 13. F11 [P1] Scroll 子系统约 1346 行但没有进入主执行链
**归属提交**：7335750
**关键文件**：
- `crates/argusflow-vision/src/scroll/*`
- `crates/argusflow-vision/src/lib.rs`
- `crates/argusflow-windows/src/input/mouse.rs`
### 现状证据
- 新增 controller/displacement/end/history/model/session 等完整框架。
- 约 1346 行代码被公开 export。
- VisionRuntime 没有消费 ScrollSession/ScrollController。
- Tauri runtime 没有装配 scroll controller。
- mouse 层只有低层 wheel injection，与 scroll session 闭环未接。
### 影响
- 公开 API 面积明显扩大，但没有产品纵向 consumer。
- 主链 transport/output 尚未稳定时先固化大块高级算法，review 与维护成本不合理。
### 为什么这是问题
不是说 scroll 算法一定错，而是实现优先级和 public surface 过早。
### 整改建议
1. 没有真实 consumer 前改为 `pub(crate)` 或 feature gate。
2. 若短期不做 semantic Scroll，考虑 revert 该能力岛。
3. 等主视觉链稳定后，以一个可执行 Scroll vertical slice 重新公开。
### 最低测试门槛
- [ ] 只有真正接 Runtime 后再要求 scroll E2E：wheel -> displacement -> overlap -> accepted page -> history。
---
## 14. F12 [P1] WorkerSupervisor 已实现，但真实 worker lifecycle ownership 留给外部部署层
**归属提交**：7335750
**关键文件**：
- `crates/argusflow-vision/src/worker/health.rs`
- `src-tauri/src/runtime.rs`
- `workers/argusflow-vision-worker/README.md`
### 现状证据
- Rust 有 WorkerRestartPolicy/WorkerSupervisor。
- Tauri 只读 pipe name/session token 环境变量。
- 未配置环境变量就使用 UnavailableOcrEngine。
- Tauri 不 spawn/prewarm/restart/shutdown worker。
- README 又明确由部署层生成 token 并启动 worker。
### 影响
- lifecycle 责任边界一半在 app、一半在“部署层”。
- 已有 supervisor 成为未接线实现。
- 开发机/最终用户启动视觉能力依赖外部隐藏步骤。
### 为什么这是问题
核心本地 worker 必须有单一 owner。
### 整改建议
1. 推荐 Desktop owns worker：generate token -> spawn -> health -> bounded restart -> shutdown。
2. 如果坚持 Deployment owns worker，就移除未使用 supervisor public surface，并提供独立 launcher/service 与兼容检查。
### 最低测试门槛
- [ ] spawn success、missing executable、model load fail、crash restart budget、app shutdown cleanup。
---
## 15. F13 [P1] WGC readback 每帧创建 D3D11 staging texture
**归属提交**：7335750
**关键文件**：
- `crates/argusflow-windows/src/capture/wgc.rs`
### 现状证据
- `readback_frame` 每次读取 source desc 后调用 `CreateTexture2D` 创建 staging。
- 随后 CopyResource/Map/CPU copy/Unmap。
- subscription 已经持有 capture_size 并能检测 resize。
### 影响
- 每帧产生 GPU/driver 资源创建和 COM churn。
- 与高频稳定帧检测/增量视觉的低延迟目标冲突。
### 为什么这是问题
staging texture 是典型 per-subscription 可复用资源。
### 整改建议
1. 把 staging texture 放进 WindowFrameSubscription state。
2. 同尺寸复用，resize 时和 frame pool 一起重建。
3. 必要时做 2～3 buffer ring，避免同步争用。
### 最低测试门槛
- [ ] steady 60s capture allocation count、resize 后重建、Map failure recovery、benchmark p50/p95。
---
## 16. F14 [P1] 局部 scene refresh 会重置全局 `stored_at`，可能人为延长未刷新区域 freshness
**归属提交**：7335750
**关键文件**：
- `crates/argusflow-vision/src/scene/cache.rs`
- `crates/argusflow-vision/src/scene/model.rs`
### 现状证据
- CacheState 只有一个 `stored_at`。
- `replace_region` 合并 base scene 后统一设置 `stored_at = now`。
- 新 scene 可能包含大量从旧 base_scene 继承、未重新 OCR 的 nodes。
### 影响
- 刷新 B 区域会让 A 区域旧节点在 age 判断上看起来也刚刷新。
- dirty 与 freshness 两个概念被一个 scene-global timestamp 混合。
### 为什么这是问题
局部更新需要 region/node 粒度的观察时间，而不是全 scene 一把时钟。
### 整改建议
1. 给 node/region 记录 observed_frame/observed_at，或维护 region freshness map。
2. 局部 refresh 只刷新覆盖区域 age。
3. cache lookup 对 query region 计算对应 freshness。
### 最低测试门槛
- [ ] A 过期+B 刷新后查询 A 仍应 Expired；B 查询可 Hit。
---
## 17. F15 [P2] Visual resolver 把 OCR/Capture/Protocol 错误都归因为 SendInput
**归属提交**：e7d0169
**关键文件**：
- `crates/argusflow-windows/src/input/visual_resolver.rs`
### 现状证据
- `WorkerUnavailable/CaptureUnavailable/OcrFailed/Protocol` 被映射到 BackendKind::SendInput。
- 真实失败发生在视觉 materialization stage，而非鼠标注入。
### 影响
- Explain、Evidence、metrics、用户日志都会把根因标错。
- 未来 fallback 逻辑可能根据错误 backend 做出错误判断。
### 为什么这是问题
这是 F05 分层错误的直接症状。
### 整改建议
1. 随 Planner materialization 重构一起删除该映射。
2. MaterializationError 保留 source backend/stage/cause。
3. SendInput 只产生 foreground/coordinate/partial injection 错误。
### 最低测试门槛
- [ ] worker unavailable 的 evidence backend 必须是 OCR/vision stage；真实 partial SendInput 才标 SendInput。
---
## 18. F16 [P2] Capture subscription 缓存 key 忽略 CapturePolicy
**归属提交**：7335750
**关键文件**：
- `crates/argusflow-vision/src/runtime.rs`
### 现状证据
- `subscription(window, policy)` 第一次按 policy open。
- 之后仅按 WindowIdentity 查找并复用。
- CapturePolicy 包含 frame_pool_size/include_cursor/max_dimension 等语义。
### 影响
- 同一窗口后续请求不同 policy 时，调用方配置可能不生效。
### 为什么这是问题
缓存 key 和实际资源构造参数不一致。
### 整改建议
1. 如果 policy 应 runtime-global，就把它移出 per-call SceneRefreshPolicy。
2. 否则 SubscriptionKey 必须包含 policy fingerprint。
### 最低测试门槛
- [ ] 同 window 不同 include_cursor/max_dimension policy 的行为必须明确。
---
## 19. F17 [P2] 视觉 bbox 到屏幕坐标映射缺少真实 Windows DPI/边框 fixture 证明
**归属提交**：e7d0169
**关键文件**：
- `crates/argusflow-windows/src/input/visual_resolver.rs`
- `crates/argusflow-windows/src/capture/wgc.rs`
### 现状证据
- 当前主要用 GetWindowRect.left/top 与 frame-local bbox 计算 screen point。
- WGC frame content rect 当前通常按 0,0,width,height 建模。
- 代码没有在该路径显式展示 client rect/extended frame/mixed DPI 的 fixture proof。
### 影响
- 在窗口边框、mixed DPI、多屏负坐标、某些 capture item geometry 下可能出现偏移。
### 为什么这是问题
静态代码不足以证明这条 Windows 几何链在所有目标环境正确。
### 整改建议
1. 把坐标变换封成 Windows-only `SurfaceTransform`，记录输入/输出 coordinate space。
2. 用真实 fixture 做 overlay/click 误差测试。
### 最低测试门槛
- [ ] 100/125/150/200% DPI、mixed monitors、negative virtual coordinates、maximized、resize、popup。
---
## 20. F18 [P2] Python deadline 是推理完成后检查，无法真正取消 Paddle predict
**归属提交**：7335750
**关键文件**：
- `workers/argusflow-vision-worker/src/argusflow_vision_worker/worker.py`
- `crates/argusflow-vision/src/worker/named_pipe.rs`
### 现状证据
- Python 先同步执行 `pipeline.predict`，结束后才比较 elapsed 与 deadline。
- Rust 外层 timeout 后会断 connection，但 Python predict 仍可能继续占 CPU/GPU。
### 影响
- 连续 timeout 可能造成 worker 仍繁忙、后续请求抖动、重连与真实计算状态脱节。
### 为什么这是问题
deadline 语义应区分“客户端等待截止”与“推理可取消”。
### 整改建议
1. P0 明确：超时即视 worker unhealthy，并按 lifecycle 策略 kill/restart，避免假装 cooperative cancel。
2. P1 再考虑独立 inference process/job cancellation。
### 最低测试门槛
- [ ] 长推理 timeout 后下一请求行为、worker health、restart budget。
---
## 21. F19 [P2] `UiPayloadV2` 同时承载 v2/v3，schema migration ownership 不够清晰
**归属提交**：e7d0169
**关键文件**：
- `crates/argusflow-runtime/src/builtin_nodes/ui.rs`
- `crates/argusflow-core/src/visual.rs`
### 现状证据
- UI node version `2 | 3` 都 deserialize 到名为 UiPayloadV2 的结构。
- v3 的视觉 ValueExpr 兼容主要藏在 VisualQueryExpr custom Deserialize。
### 影响
- 未来 v4 容易继续把 schema 演进堆进 serde magic，版本语义越来越难审。
### 为什么这是问题
功能当前可用，但 migration 应显式形成 old -> current internal contract。
### 整改建议
1. 把内部结构改名 `CurrentUiPayload`，或显式 UiPayloadV2/UiPayloadV3 + migrate。
2. 为每个历史版本保留 fixture migration test。
### 最低测试门槛
- [ ] v1/v2/v3 fixture 全部 decode 到同一 current semantic action；重新 encode 只输出 current version。
---
## 22. F20 [P2] 节点 Palette 展示多个 `kind: null` 的未来节点，UI 能力契约不够明确
**归属提交**：e7d0169
**关键文件**：
- `src/components/workflow/palette/nodePaletteCatalog.ts`
### 现状证据
- 并行执行、循环执行、脚本处理、筛选/合并/整理字段等条目为 `kind: null`。
- 它们与真实可创建节点处于同一节点页目录。
### 影响
- 用户容易把目录条目理解为当前已支持能力。
- 产品能力表与 Runtime registry 可能产生认知漂移。
### 为什么这是问题
Studio 节点库最好只展示真实可拖入能力；roadmap 需要明确状态。
### 整改建议
1. 未实现项默认隐藏，或显示显式 `即将支持` badge 且 disabled。
2. 最好从 Runtime/registry capability 派生可用性，而不是手工 `null`。
### 最低测试门槛
- [ ] Palette 中 enabled item 必须能 resolveCreationKind/createNode；disabled item 明确不可拖。
---
## 23. 值得保留的实现：不要因为返工把正确边界一起推翻
### NormalizedRect 不变量
- private fields + new() + custom Deserialize，finite、>0、unit viewport 边界在输入层锁住。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### VisualQueryExpr 接 ValueExpr
- 视觉文字目标可以来自 workflow input/variable/node output，并继续走统一引用与类型系统。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### 旧 string visual query 兼容
- 历史 payload 可在 decode 时转换成 literal ValueExpr，迁移方向合理。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### 严格 0/1/N
- 0=TargetNotFound、1=Unique、N=Ambiguous，没有用最高 confidence 隐式执行。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### fuzzy 只提供候选
- 相似度排序不等于自动点击，安全边界正确。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### HWND/PID 双重身份
- WGC open 时复验 IsWindow 与 PID，降低 HWND reuse 跨进程风险。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### PixelImage 不 Serialize
- 避免默认 evidence/log 误存原始像素，隐私与生命周期意识正确。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### SendInput partial injection 检测
- Windows 只接受部分事件时不误报成功。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### 点位在窗口内复验
- 坐标 click 前确认 point 仍属于目标 window。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### Preset 不新增 Runtime type
- NodePreset 最终仍保存普通 argus.ui，符合 docs 的 Primitive/Preset 分层。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### 固定暂停文案
- Delay 明确表达为固定暂停，不再拿它做 UI readiness。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### 默认流程删除 Delay
- 用 target wait 与视觉确认替代猜时间的 fixed sleep，方向正确。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### VisionRuntime 共享实例
- VisualCache/Tiny/Medium 意图上共用 capture/worker/cache，而不是三套 runtime。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
### Named Pipe session token
- 本地 worker 有最基础的会话鉴权与 request correlation。
- 建议：保留现有思想，只修生命周期、路由或测试缺口。
## 24. 根因分析
### 24.1 把模块完成度当作链路完成度
Capture/Diff/Scene/OCR/Scroll 都“有文件”并不等于 Workflow -> Output 已可用。
- 改进原则：先建立一个最小 vertical slice，再横向扩能力。
- Review 必须回答：公开输入到可观察输出的每个 contract 是否被测试。
### 24.2 缺少跨 crate contract tests
最严重的 output key 错误就是 Runtime 与 Vision 各自 unit test 无法发现的。
- 改进原则：先建立一个最小 vertical slice，再横向扩能力。
- Review 必须回答：公开输入到可观察输出的每个 contract 是否被测试。
### 24.3 P0 cut line 不明确
主链尚未跑通就同时推进 Scroll、Topology、Metrics、Evidence、Studio assets。
- 改进原则：先建立一个最小 vertical slice，再横向扩能力。
- Review 必须回答：公开输入到可观察输出的每个 contract 是否被测试。
### 24.4 局部快速打通破坏全局架构
Visual Click 为了尽快可用，把 OCR materialization 塞进 SendInput，形成第二套路由。
- 改进原则：先建立一个最小 vertical slice，再横向扩能力。
- Review 必须回答：公开输入到可观察输出的每个 contract 是否被测试。
### 24.5 把“接口存在”误认为“能力实现”
有 WindowTopologyTracker 不等于 OwnedPopup pixels 已进入 OCR surface。
- 改进原则：先建立一个最小 vertical slice，再横向扩能力。
- Review 必须回答：公开输入到可观察输出的每个 contract 是否被测试。
### 24.6 生命周期靠注释维护
VisualResolved 明明 runtime-only，却没有类型层禁止持久化。
- 改进原则：先建立一个最小 vertical slice，再横向扩能力。
- Review 必须回答：公开输入到可观察输出的每个 contract 是否被测试。
### 24.7 同一提交同时放大 code + docs
1500 行设计文档和几十个实现文件同 diff，会显著降低 reviewer 发现跨层细节问题的概率。
- 改进原则：先建立一个最小 vertical slice，再横向扩能力。
- Review 必须回答：公开输入到可观察输出的每个 contract 是否被测试。
### 24.8 没有先写失败场景
如果先写“大 ROI 超 4MB”“历史同文不能验证新 send”测试，两个核心问题很早就会暴露。
- 改进原则：先建立一个最小 vertical slice，再横向扩能力。
- Review 必须回答：公开输入到可观察输出的每个 contract 是否被测试。
## 25. 推荐目标架构
目标不是再造更复杂的框架，而是让已有 `Workflow -> PreparedPlan -> Backend -> Evidence` 成为唯一主骨架。
### 25.1 三种生命周期类型必须分开
- Persisted：`AutomationTarget + TargetLocator::Visual { VisualQueryExpr }`。
- Prepared：`PreparedAutomationTarget + PreparedLocator::Visual { VisualQuery }`。
- Materialized：`MaterializedTarget::ScreenPoint { window, point, bounds, scene_id, frame_id, generation, source_backend }`。
不要再让 persisted enum 同时承担 prepared state。
### 25.2 推荐执行链
```text
Workflow Primitive
  -> UiNode compile / ValueExpr resolve
  -> Prepared Semantic Action
  -> ActionRouter / PreparedPlan
       -> semantic fast path: UIA / CDP
       -> target materialization: VisualCache -> OcrTiny -> OcrMedium -> Grounding
  -> MaterializedTarget
  -> SendInput actuator
  -> verification / evidence
```
### 25.3 Planner 负责
- support level
- backend availability
- cost
- user prefer/deny policy
- fallback ordering
- shared deadline
- target wait
- evidence capture
- error attribution
- retry eligibility
### 25.4 VisionRuntime 负责
- window-scoped capture
- stable frame
- dirty detection
- refresh planning
- OCR request
- scene merge
- scene freshness
- visual query
- verification facts
- vision metrics
### 25.5 SendInput 只负责
- foreground revalidation
- HWND/PID identity revalidation
- point/window bounds validation
- physical input injection
- partial injection failure
SendInput 不负责：OCR tier 选择、VisualCache fallback、fuzzy ranking、scene refresh policy。
### 25.6 Per-scope Vision state
建议内部结构：
```text
VisionRuntime
  scopes: HashMap<VisualScopeKey, ScopeState>
ScopeState
  subscription
  current_scene
  last_stable_frame
  temporal_noise
  region_freshness
  last_access
  scope_metrics
```
- Scope key 至少包含稳定 WindowIdentity；未来真正 multi-surface 时升级为 AppSession/surface identity。
- 必须有 bounded LRU/idle TTL，防止窗口历史无限保留。
- 关闭/失效 subscription 必须 prune。
- CapturePolicy 要么变 runtime-global，要么成为 subscription key 的一部分。
### 25.7 RefreshPlan
建议结构：
```text
RefreshPlan
  CacheOnly
  Partial { regions, coverage_ratio, reason }
  Full { reason }
```
- Topology change -> Full。
- 无 base scene -> Full。
- Dirty coverage 超阈值 -> Full。
- Dirty 小且 query 有交集 -> Partial。
- Query region 无 dirty -> CacheOnly。
- Partial refresh 只刷新覆盖 region 的 freshness。
### 25.8 Worker protocol
推荐：控制面 JSON + 像素二进制面。
```text
frame header:
  magic
  protocol version
  json_header_len
  binary_body_len
  JSON metadata
  raw BGRA body
```
- JSON 只放 request/window/frame/generation/roi/model/deadline。
- 像素 body 用 raw bytes 或 shared memory lease。
- Python 使用 memoryview/numpy.frombuffer。
- 严格检查 body length 与 stride*height。
- session token/request id/version 继续保留。
### 25.9 非幂等 verification
推荐状态不是二态，而是：
- Success：明确观察到本次动作的新事实。
- Failure：明确观察到失败事实。
- Unknown：动作已提交，但无法证明结果，禁止自动重试。
## 26. 推荐整改 PR 顺序
原则：每个 PR 都必须是可 review 的纵向 slice；不要再把下一轮 50 个文件改动叠到当前 main 上。
### PR-1：锁 Vision ActionOutcome 输出契约
**本 PR 做：**
- 修 GetText/Text。
- Vision GetValue 暂时 Unsupported。
- Extract One/Many 分别 item/items。
- 新增跨 Backend output contract test。
**本 PR 不做：**
- 不改 IPC。
- 不改 Planner。
- 不加新视觉能力。
**验收：**
- [ ] 所有声明 output port 与真实 ActionOutcome keys 一致。
### PR-2：替换 OCR JSON 像素数组
**本 PR 做：**
- 定义 protocol v2。
- JSON header 与 raw binary body 分离，或直接 shared memory。
- Python 改 numpy.frombuffer。
- 保留 token/version/request correlation。
**本 PR 不做：**
- 不顺便改 Scene 算法。
- 不顺便改 UI。
**验收：**
- [ ] 1200×800 典型 ROI 可稳定传输。
- [ ] 高熵像素不因十进制编码膨胀。
### PR-3：DirtyMap -> RefreshPlan
**本 PR 做：**
- 计算 dirty 后生成 CacheOnly/Partial/Full。
- query region 与 dirty 做交集。
- partial merge 更新 scene。
- 新增 OCR pixel metrics。
**本 PR 不做：**
- 不做 Scroll。
- 不做 Grounding。
**验收：**
- [ ] idle/typing 场景 ocr_pixels 显著低于 captured_pixels。
### PR-4：拆 persisted target 与 prepared target
**本 PR 做：**
- 移除 TargetLocator::VisualResolved。
- 新增 PreparedTargetLocator。
- ValueExpr 在 prepare 解析。
- 新增 schema migration fixtures。
**本 PR 不做：**
- 不改产品 UI 字段。
**验收：**
- [ ] Workflow JSON 永不出现 visual_resolved。
### PR-5：Planner-owned visual materialization
**本 PR 做：**
- Cache/Tiny/Medium 进入统一 materializer candidate chain。
- Explain 展示 materializer stage。
- MaterializedTarget 携带 scene/frame/generation。
- SendInput 只做 actuator。
**本 PR 不做：**
- 不在 SendInput 里选择 OCR model。
**验收：**
- [ ] cache unique 不 OCR；tiny unique 不 medium；错误 backend attribution 正确。
### PR-6：Canonical WeChat Component + 安全 send verification
**本 PR 做：**
- 唯一 createWechatMessageDefinition。
- 默认模板从 Component 派生。
- pre-send scene token。
- post-send delta。
- Unknown outcome 禁止 retry。
**本 PR 不做：**
- 不加更多业务 Component。
**验收：**
- [ ] 历史同文本不能误判成功。
### PR-7：VisionRuntime per-scope state
**本 PR 做：**
- HashMap scope state。
- cache/frame/noise/freshness 按 scope。
- subscription prune。
- LRU/TTL。
**本 PR 不做：**
- 不同时实现完整 OwnedPopup surface set。
**验收：**
- [ ] A/B/A 查询不互相清 cache。
### PR-8：处理 Scroll/OwnedPopup/WGC 性能债
**本 PR 做：**
- 决定 Scroll feature-gate 还是正式接 Runtime。
- 明确 OwnedPopup P0 能力边界。
- 复用 staging texture。
- 完善 DPI/multi-monitor fixture。
**本 PR 不做：**
- 不在未有测试时扩 public API。
**验收：**
- [ ] docs 与真实支持范围一致。
## 27. PR-1 具体实现建议：统一输出 Shape
当前 bug 的根因之一是 Backend 自己手写字符串。
建议建立内部统一映射：
```text
AutomationAction::GetText       -> Text("text")
AutomationAction::GetValue      -> Value("value")
AutomationAction::Extract One   -> Json("item")
AutomationAction::Extract Many  -> Json("items")
Click/PressKey/TypeText         -> None
```
- Runtime 的 `value_output()` 从同一 shape 生成。
- Backend ActionOutcome encoder 也从同一 shape 生成。
- 这样 schema 与 executor 不再各写一份 key。
- 如果某 Backend 不支持语义，必须在 prepare/capability 阶段拒绝，而不是输出不同 key。
### 27.1 GetValue 的策略
- UIA：如果 ValuePattern/等价能力成立，支持。
- CDP：按 DOM property/value 语义支持。
- Vision：P0 Unsupported。
- 未来 Grounding：除非模型返回可验证 value fact，否则同样 Unsupported。
### 27.2 Extract 的策略
- `One` 必须唯一命中，输出 object 到 `item`。
- `Many` 可以 0..N，输出 array 到 `items`。
- 字段 source 为 Text/Name 时 Vision 可支持。
- Attribute/Property/Value 没有事实来源时明确 Unsupported。
## 28. PR-2 具体实现建议：Protocol v2
不建议把当前 v1 继续兼容像素 JSON 数组作为默认路径。
可以保留 v1 只用于小 fixture/迁移，但生产 recognize 使用 v2 binary。
### 28.1 Header 必须包含
- protocol_version
- request_id
- session_token
- window identity
- frame_id
- topology_generation
- ROI
- pixel format
- width/height/stride
- body length
- OCR profile
- deadline
### 28.2 Binary 校验
- [ ] `width > 0 && height > 0`。
- [ ] `stride >= width * bpp`。
- [ ] `stride * height` checked overflow。
- [ ] `body_length == expected`，不要接受“至少这么长”再默默截断。
- [ ] 设置独立 `MAX_PIXEL_BODY_BYTES`。
- [ ] 拒绝未知 pixel format。
### 28.3 Python 零多余对象路径
```text
raw bytes/memoryview
-> numpy.frombuffer(dtype=uint8)
-> reshape(height, stride)
-> slice width*4
-> color conversion/view/copy as Paddle requires
```
禁止恢复成 `list[int]`。
## 29. PR-3 具体实现建议：RefreshPlan
建议 `update_cache_invalidation` 不再只做 side effect。
它应返回可解释 plan。
```text
observe stable frame
-> compute dirty
-> temporal noise filter
-> merge/pad regions
-> intersect requested query region
-> choose CacheOnly/Partial/Full
-> OCR effective regions
-> merge scene
-> update region freshness
```
### 29.1 Full 条件
- 没有历史 scene。
- window/surface identity 变化。
- topology generation 变化。
- frame dimensions/coordinate transform generation 变化。
- dirty coverage 超 major transition threshold。
- 调用方显式要求 full verification。
### 29.2 Partial 条件
- 有可用 base scene。
- dirty coverage 小。
- query region 与 dirty 相交。
- 历史 region freshness 仍允许保留未覆盖 nodes。
### 29.3 Metrics
- `capture_pixels`
- `dirty_pixels`
- `query_pixels`
- `ocr_pixels`
- `full_refresh_count`
- `partial_refresh_count`
- `cache_only_count`
- `scene_nodes_reused`
- `scene_nodes_refreshed`
## 30. PR-4/5 具体实现建议：Prepared Target 与 Materialization
### 30.1 Persisted
Workflow 只保存用户语义：
```text
AutomationTarget
  scope
  locator: Query | VisualExpr | Coordinate | Focused
  backend_policy
```
### 30.2 Prepared
compile/prepare 后：
```text
PreparedAutomationTarget
  frozen scope
  locator: PreparedQuery | VisualQuery | Coordinate | Focused
  policy
```
### 30.3 Materialized
视觉/语义定位成功后：
```text
MaterializedTarget::ScreenPoint
  window
  bounds
  safe_point
  source_backend
  scene_id
  frame_id
  topology_generation
  confidence
```
### 30.4 Materializer candidate 行为
- **VisualCache hit unique**：直接 materialize；不 OCR。
- **VisualCache miss**：进入 Tiny。
- **Tiny unique**：materialize；不 Medium。
- **Tiny not found**：若 deadline 允许，可等待/重新观察；是否升级由 policy。
- **Tiny ambiguous**：默认立即 Ambiguous；不能靠 Medium 自动挑一个，除非明确设计为“提高识别质量后重新判唯一性”。
- **Tiny low-confidence事实**：可以作为显式升级 Medium 的理由，但必须进入 Explain。
- **Medium unique**：materialize。
- **Medium ambiguous**：失败。
- **WorkerUnavailable**：backend unavailable，按 Planner fallback。
- **CaptureUnavailable**：materialization stage failure，不冒充 SendInput。
### 30.5 Actuator 行为
- [ ] 检查 materialized window 与当前 HWND/PID 一致。
- [ ] 确认窗口仍 foreground 或按 activation policy 激活。
- [ ] 确认 point 仍在允许 bounds。
- [ ] 必要时检查 materialization age/generation。
- [ ] 调用 SendInput。
- [ ] partial injection 必须失败。
## 31. PR-6 具体实现建议：Canonical WeChat Component
### 31.1 单一 graph source
建议把业务定义移到类似：
```text
src/features/workflow/components/builtin/wechatMessage.ts
```
只导出一个 versioned definition factory。
### 31.2 默认模板
- 产品若允许组件节点展示：默认模板只放 Start -> WeChat Component -> End。
- 产品若希望首次打开就看到内部流程：由 canonical Component definition 做确定性 expansion，不能再手写第二份 nodes/edges。
- ApplicationSpec、visual regions、wait policy 都从同一组件模块导出。
### 31.3 搜索与群聊验证
- 搜索 ready 不能仅靠过宽 region 内的“包含 搜索”长期作为最终 fixture；需要真实微信版本验证。
- 找群继续 strict exact + bounded region；同名群必须 Ambiguous。
- 点击后 header exact group name 是合理 post-condition。
### 31.4 发送前观察
- [ ] 记录当前 scene_id/frame_id。
- [ ] 记录 message region 中与发送文本相同的 stable hashes。
- [ ] 记录当前 bottom row/anchor。
- [ ] 记录 header 仍是目标群。
### 31.5 发送后判断
- Success：出现 pre-send 集合里没有的新 matching row。
- Success：new row 属于新 scene/generation。
- Success：位置与 appended message 一致。
- Failure：明确出现发送失败事实。
- Unknown：无法证明新增，也无法证明失败。
- Unknown：绝不自动再次 Enter。
## 32. PR-7 具体实现建议：Per-scope VisionRuntime
### 32.1 ScopeState
- `subscription`
- `scene_cache`
- `last_stable_frame`
- `temporal_noise`
- `region_freshness`
- `last_access`
- `scope_health`
### 32.2 生命周期
- [ ] WindowIdentity 失效时立即移除 subscription。
- [ ] AppSession 释放时主动释放 scope state。
- [ ] 超 idle TTL 的 state 可 LRU eviction。
- [ ] worker 是共享的，但 scene/frame/noise 不是共享单例。
- [ ] metrics 同时支持 global aggregate 与 per-scope tag。
## 33. PR-8：Scroll / OwnedPopup / WGC 性能债的决策门
### 33.1 Scroll
只有满足下面任一条件才继续公开扩展：
- [ ] 已经有正式 `UiOperation::Scroll` semantic primitive consumer。
- [ ] 已有一个 versioned Component 依赖 ScrollController。
- [ ] 已有 Windows fixture 验证 wheel->displacement->overlap->history 闭环。
否则：
- 改为 crate-private。
- 或 feature gate。
- 或 revert 后以后按 vertical slice 重新提交。
### 33.2 OwnedPopup
必须选择一个真实产品承诺：
- P0 primary HWND only；popup 只触发 cache invalidation。
- 或实现真正 VisualSurfaceSet，多 HWND capture/query/materialize。
### 33.3 WGC
- [ ] 复用 staging texture。
- [ ] resize 重建 pool + staging。
- [ ] 记录 frame readback latency。
- [ ] 建立连续 capture benchmark，观察 allocation count。
## 34. 测试体系整改
本次最严重问题都跨越模块边界，因此单元测试不是主要缺口的替代品。
### 34.1 五层测试
- **Unit**：NormalizedRect、DirtyMap、StableFrameGate、0/1/N、scroll algorithm、protocol DTO。
- **Contract**：Runtime output port vs Backend ActionOutcome、persisted vs prepared serde、error taxonomy、capability truthfulness。
- **Integration**：Rust VisionRuntime + fake capture + fake/real protocol worker。
- **Windows Fixture**：WGC、DPI、resize、popup、SendInput、foreground。
- **Simulated E2E**：用可控 fixture app 跑完整 WeChat-like search/send/verify flow。
### 34.2 为什么需要 simulated WeChat fixture
- 真实微信不适合 CI，版本、账号、网络都不可控。
- 但我们需要可控复现延迟渲染、重复文本、popup、发送失败、焦点丢失。
- fixture app 可以画与微信相似的区域和文字，同时把内部事件暴露给测试断言。
## 35. 必须进入 CI 的回归场景
### T001 Vision GetText 唯一命中
- 输入/前置：构造单 OCR node。
- 期望：ActionOutcome 只有 `text`。
- 防回归重点：Runtime output descriptor 同名。
### T002 Vision GetValue
- 输入/前置：视觉目标唯一。
- 期望：返回 Unsupported。
- 防回归重点：不得返回 `text` 冒充 value。
### T003 Vision Extract One
- 输入/前置：唯一 node + Text field。
- 期望：ActionOutcome 只有 `item`。
- 防回归重点：item 为 object。
### T004 Vision Extract Many
- 输入/前置：多个 node + Text field。
- 期望：ActionOutcome 只有 `items`。
- 防回归重点：items 为 array。
### T005 Visual Extract Value field
- 输入/前置：请求 FieldProjectionSource::Value。
- 期望：Unsupported。
- 防回归重点：不得伪造 OCR text。
### T006 旧 visual string schema
- 输入/前置：v2 fixture text=string。
- 期望：迁移成 literal ValueExpr。
- 防回归重点：重新编码 current schema。
### T007 新 visual ValueExpr
- 输入/前置：workflow_input ref。
- 期望：prepare 解析本次字符串。
- 防回归重点：persisted 数据不变。
### T008 非法 NormalizedRect
- 输入/前置：x+width>1。
- 期望：deserialize fail。
- 防回归重点：无效 rect 不进入 runtime。
### T009 Persisted visual_resolved
- 输入/前置：手写 JSON type=visual_resolved。
- 期望：schema reject。
- 防回归重点：runtime-only 类型不可 serde。
### T010 0 target
- 输入/前置：空 scene。
- 期望：TargetNotFound。
- 防回归重点：target wait 才可恢复。
### T011 1 target
- 输入/前置：唯一 exact node。
- 期望：Unique。
- 防回归重点：允许 materialize。
### T012 N target
- 输入/前置：两个 exact node。
- 期望：AmbiguousTarget。
- 防回归重点：不得点击最高 confidence。
### T013 fuzzy candidates
- 输入/前置：两个相似 node。
- 期望：返回排序候选。
- 防回归重点：不得自动执行第一项。
### T014 Cache unique click
- 输入/前置：新鲜 cache 唯一。
- 期望：materialize 后 SendInput。
- 防回归重点：OCR call count=0。
### T015 Cache miss Tiny unique
- 输入/前置：cache miss。
- 期望：Tiny OCR 唯一后点击。
- 防回归重点：Medium call count=0。
### T016 Tiny ambiguity
- 输入/前置：Tiny 两个 exact。
- 期望：Ambiguous 或显式升级策略。
- 防回归重点：不能静默选一个。
### T017 Medium unique
- 输入/前置：策略要求升级。
- 期望：Medium 唯一。
- 防回归重点：Explain 记录升级原因。
### T018 Worker unavailable
- 输入/前置：OCR worker 未连接。
- 期望：Materialization backend unavailable。
- 防回归重点：错误不标 SendInput。
### T019 Capture unavailable
- 输入/前置：WGC open 失败。
- 期望：Materialization failure。
- 防回归重点：错误不标 SendInput。
### T020 SendInput partial
- 输入/前置：系统只接受部分 INPUT。
- 期望：BackendFailed(SendInput)。
- 防回归重点：不得报告成功。
### T021 Foreground changed
- 输入/前置：materialize 后切窗口。
- 期望：actuation 前失败/重新 materialize。
- 防回归重点：不能点到别的 app。
### T022 HWND PID changed
- 输入/前置：同 handle 新 PID。
- 期望：WindowIdentityChanged。
- 防回归重点：拒绝 capture/click。
### T023 Point outside window
- 输入/前置：materialized point 越界。
- 期望：PointOutsideWindow。
- 防回归重点：不注入。
### T024 Virtual screen negative
- 输入/前置：副屏在左侧。
- 期望：归一化坐标正确。
- 防回归重点：不截断负物理坐标。
### T025 DPI 100%
- 输入/前置：fixture button。
- 期望：点击 bbox center。
- 防回归重点：误差阈值内。
### T026 DPI 125%
- 输入/前置：fixture button。
- 期望：点击 bbox center。
- 防回归重点：误差阈值内。
### T027 DPI 150%
- 输入/前置：fixture button。
- 期望：点击 bbox center。
- 防回归重点：误差阈值内。
### T028 Mixed DPI
- 输入/前置：双屏不同 scale。
- 期望：正确 surface transform。
- 防回归重点：目标屏正确。
### T029 Window resize
- 输入/前置：capture 中 resize。
- 期望：pool/staging 重建。
- 防回归重点：frame dimensions 正确。
### T030 WGC steady allocation
- 输入/前置：固定尺寸连续帧。
- 期望：staging 复用。
- 防回归重点：每帧不 CreateTexture2D。
### T031 Protocol 256x256
- 输入/前置：BGRA binary。
- 期望：成功 OCR request。
- 防回归重点：body exact length。
### T032 Protocol 800x600
- 输入/前置：BGRA binary。
- 期望：成功。
- 防回归重点：不受 JSON byte 膨胀。
### T033 Protocol 1200x800 50% ROI
- 输入/前置：典型消息区域。
- 期望：成功。
- 防回归重点：不超过 control JSON limit。
### T034 Protocol high entropy
- 输入/前置：随机像素。
- 期望：成功。
- 防回归重点：body size=raw size。
### T035 Protocol stride padding
- 输入/前置：stride>width*4。
- 期望：正确解析每行。
- 防回归重点：无错位。
### T036 Protocol truncated body
- 输入/前置：少一个 byte。
- 期望：Protocol error。
- 防回归重点：不读越界。
### T037 Protocol oversized body
- 输入/前置：超过 MAX_PIXEL_BODY。
- 期望：拒绝。
- 防回归重点：bounded allocation。
### T038 Protocol wrong token
- 输入/前置：token mismatch。
- 期望：Unauthorized。
- 防回归重点：不进入 OCR。
### T039 Protocol wrong version
- 输入/前置：vX。
- 期望：ProtocolMismatch。
- 防回归重点：连接恢复策略明确。
### T040 Protocol wrong request_id
- 输入/前置：响应错配。
- 期望：reject response。
- 防回归重点：不写 scene。
### T041 Worker timeout
- 输入/前置：predict 超 deadline。
- 期望：timeout/worker recovery。
- 防回归重点：下一请求行为确定。
### T042 Worker crash
- 输入/前置：子进程退出。
- 期望：bounded restart。
- 防回归重点：超过预算进入 Failed。
### T043 Worker prewarm fail
- 输入/前置：模型加载失败。
- 期望：health=Failed。
- 防回归重点：Planner availability unavailable。
### T044 Worker reconnect
- 输入/前置：pipe 断开重建。
- 期望：重新握手。
- 防回归重点：旧 response 不复用。
### T045 Refresh first scene
- 输入/前置：无 base scene。
- 期望：Full。
- 防回归重点：建立完整初始 scene。
### T046 Refresh no dirty
- 输入/前置：前后稳定相同。
- 期望：CacheOnly。
- 防回归重点：OCR pixels=0。
### T047 Refresh one dirty tile
- 输入/前置：局部变化。
- 期望：Partial。
- 防回归重点：OCR 只覆盖小 ROI。
### T048 Refresh many nearby
- 输入/前置：多个近邻 dirty。
- 期望：合并 partial。
- 防回归重点：避免过多 worker calls。
### T049 Refresh high coverage
- 输入/前置：大面积变化。
- 期望：Full。
- 防回归重点：reason=major transition。
### T050 Refresh query outside dirty
- 输入/前置：query 区域未变化。
- 期望：CacheOnly。
- 防回归重点：不因别处 dirty OCR query。
### T051 Refresh query intersects dirty
- 输入/前置：局部相交。
- 期望：OCR intersection/merged ROI。
- 防回归重点：不整帧。
### T052 Topology change
- 输入/前置：popup/identity set 变化。
- 期望：Full/invalidated。
- 防回归重点：旧 scene 不复用。
### T053 Stale frame response
- 输入/前置：response frame_id 旧。
- 期望：OcrCancelled。
- 防回归重点：不得 merge。
### T054 Stale generation response
- 输入/前置：generation 旧。
- 期望：OcrCancelled。
- 防回归重点：不得 merge。
### T055 Partial freshness A/B
- 输入/前置：A 旧 B 刷新。
- 期望：A 仍按原 age。
- 防回归重点：B 可 fresh。
### T056 A/B/A windows
- 输入/前置：两个 app 轮询。
- 期望：各自 cache 保持。
- 防回归重点：不互相 clear。
### T057 Scope eviction
- 输入/前置：超过 LRU/TTL。
- 期望：最旧 state 回收。
- 防回归重点：活动 scope 不误删。
### T058 Closed subscription
- 输入/前置：窗口关闭。
- 期望：subscription prune。
- 防回归重点：重开新 identity。
### T059 CapturePolicy reuse
- 输入/前置：同 window 改 policy。
- 期望：行为按设计明确。
- 防回归重点：不能静默沿用旧 policy。
### T060 Owned popup primary-only mode
- 输入/前置：出现 popup。
- 期望：明确 unsupported/capture limitation。
- 防回归重点：docs/Explain 一致。
### T061 Owned popup surface-set mode
- 输入/前置：popup 含唯一文字。
- 期望：可 query popup。
- 防回归重点：surface identity 保留。
### T062 Cross-surface ambiguity
- 输入/前置：primary/popup 同文。
- 期望：Ambiguous。
- 防回归重点：不得优先 primary 隐式点击。
### T063 Target wait appears early
- 输入/前置：目标 200ms 出现。
- 期望：立即成功。
- 防回归重点：不固定睡满 timeout。
### T064 Target wait timeout
- 输入/前置：目标一直不存在。
- 期望：TargetWaitTimeout。
- 防回归重点：总 deadline 准确。
### T065 Ambiguous during wait
- 输入/前置：一开始两个目标。
- 期望：立即失败。
- 防回归重点：不等待消歧。
### T066 Backend unavailable during wait
- 输入/前置：Tiny worker down。
- 期望：走 fallback/失败。
- 防回归重点：不当 TargetNotFound poll。
### T067 Shared deadline
- 输入/前置：多个 candidate。
- 期望：总耗时不乘 candidate 数。
- 防回归重点：deadline 单一。
### T068 Prepare once
- 输入/前置：target wait 多轮。
- 期望：query/plan 只 prepare 一次。
- 防回归重点：poll 只 materialize。
### T069 Pre-send duplicate
- 输入/前置：历史已有相同 message。
- 期望：记录 pre set。
- 防回归重点：旧 node 不计新增。
### T070 Send succeeds
- 输入/前置：Enter 后新 row。
- 期望：Success。
- 防回归重点：new hash/generation。
### T071 Send fails old text visible
- 输入/前置：Enter 未发送。
- 期望：不能 Success。
- 防回归重点：返回 Failure/Unknown。
### T072 Send observation unavailable
- 输入/前置：动作已提交后 capture fail。
- 期望：Unknown。
- 防回归重点：不得重发。
### T073 Send explicit fail indicator
- 输入/前置：fixture 显示发送失败。
- 期望：Failure。
- 防回归重点：不误 Unknown。
### T074 Header changed before send
- 输入/前置：目标群被切换。
- 期望：precondition fail。
- 防回归重点：不发送。
### T075 Canonical component
- 输入/前置：默认模板与 catalog。
- 期望：同源 definition。
- 防回归重点：无两份 graph。
### T076 Component version pin
- 输入/前置：实例 1.0.0。
- 期望：按精确版本展开。
- 防回归重点：升级显式。
### T077 Preset serialization
- 输入/前置：preset click。
- 期望：最终保存 argus.ui。
- 防回归重点：无 preset runtime type。
### T078 Palette implemented item
- 输入/前置：enabled item。
- 期望：resolve/create 成功。
- 防回归重点：能力目录与 registry 对齐。
### T079 Palette future item
- 输入/前置：disabled item。
- 期望：不可拖。
- 防回归重点：有即将支持状态。
### T080 Scroll API consumer gate
- 输入/前置：无 semantic consumer。
- 期望：不公开稳定 API。
- 防回归重点：feature gate/internal。
## 36. Code Review Gate：以后视觉基础设施 PR 必须回答
- [ ] R01 这个 PR 的 public entry 是什么？
- [ ] R02 这个 PR 的 observable output 是什么？
- [ ] R03 从 entry 到 output 是否有一条真实可执行 vertical slice？
- [ ] R04 有没有新增没人调用的 public type？
- [ ] R05 有没有把 runtime-only state 放入 persisted schema？
- [ ] R06 有没有新增第二套 Router/selector/wait DSL？
- [ ] R07 Backend 宣称的 capability 是否等于真实语义？
- [ ] R08 每个 output key 是否来自统一 contract？
- [ ] R09 每个错误是否保留真实 source backend/stage？
- [ ] R10 TargetNotFound 与 BackendUnavailable 是否严格区分？
- [ ] R11 Ambiguous 是否立即失败？
- [ ] R12 fuzzy 是否仅提供候选而不自动执行？
- [ ] R13 target wait 是否共享单一总 deadline？
- [ ] R14 poll 是否重复同一 PreparedPlan 而非重新 plan？
- [ ] R15 像素/二进制数据是否错误进入 JSON number array？
- [ ] R16 IPC 是否有最大长度和 checked arithmetic？
- [ ] R17 worker timeout 后真实计算状态是否可解释？
- [ ] R18 capture resource 是否每帧重复分配？
- [ ] R19 cache freshness 是否与 partial update 粒度一致？
- [ ] R20 多 window state 是否有正确 key？
- [ ] R21 subscription 是否有 prune/close 生命周期？
- [ ] R22 CapturePolicy 是否参与资源 identity？
- [ ] R23 DPI/coordinate space 是否有明确类型/测试？
- [ ] R24 OwnedPopup 是检测到，还是实际 capture 到？
- [ ] R25 非幂等动作是否定义 Success/Failure/Unknown？
- [ ] R26 retry 是否与 idempotency 一致？
- [ ] R27 post-condition 是否证明本次动作的新事实？
- [ ] R28 Component 是否真的消除了业务图复制？
- [ ] R29 Preset 是否仍只预填 Primitive？
- [ ] R30 未实现 Studio item 是否清楚标记不可用？
- [ ] R31 docs 写的能力是否真的有 integration test？
- [ ] R32 是否先写失败测试再加实现？
- [ ] R33 是否把大设计文档与大实现混在一个 diff 降低 review 信噪比？
- [ ] R34 这个 PR 是否能拆成更小但仍可执行的 slice？
## 37. 负责人验收 Checklist
- [ ] P0：Vision GetText/Extract output contract 已锁。
- [ ] P0：Vision GetValue 不再伪造 OCR text。
- [ ] P0：生产 OCR 不再通过 JSON `list[int]` 传像素。
- [ ] P0：普通 1200×800 级 ROI 可通过 protocol integration test。
- [ ] P0：DirtyMap 真正产生 OCR RefreshPlan。
- [ ] P0：Visual Click 不再由 SendInput 隐藏决定 medium OCR。
- [ ] P0：persisted workflow 不可能出现 VisualResolved。
- [ ] P0：发送历史同文本不能验证本次 send 成功。
- [ ] P0：Unknown outcome 不自动 resend。
- [ ] P1：默认微信 Workflow 与 Component 共用 canonical graph。
- [ ] P1：VisionRuntime scene/frame/noise state 按 scope。
- [ ] P1：OwnedPopup 支持范围与 docs 一致。
- [ ] P1：Scroll 无 consumer 时不暴露稳定 public API。
- [ ] P1：Worker lifecycle 有唯一 owner。
- [ ] P1：WGC staging texture 可复用。
- [ ] P1：partial refresh 不刷新其它区域 freshness。
- [ ] P2：Visual error 不再冒充 SendInput。
- [ ] P2：CapturePolicy caching 语义明确。
- [ ] P2：DPI/multi-monitor fixture 进入 CI 或 nightly。
- [ ] P2：历史 schema migration fixture 固化。
## 38. 证据文件索引
- **输出契约**：`crates/argusflow-runtime/src/builtin_nodes/ui.rs`、`crates/argusflow-vision/src/backend.rs`
- **视觉 persisted/prepared**：`crates/argusflow-core/src/automation.rs`、`crates/argusflow-core/src/visual.rs`
- **视觉 Runtime**：`crates/argusflow-vision/src/runtime.rs`
- **Scene cache**：`crates/argusflow-vision/src/scene/cache.rs`
- **Scene model**：`crates/argusflow-vision/src/scene/model.rs`
- **视觉查询**：`crates/argusflow-vision/src/query.rs`
- **Worker DTO**：`crates/argusflow-vision/src/worker/protocol.rs`
- **Worker framing**：`crates/argusflow-vision/src/worker/client.rs`
- **Named Pipe**：`crates/argusflow-vision/src/worker/named_pipe.rs`
- **Python framing**：`workers/argusflow-vision-worker/src/argusflow_vision_worker/protocol.py`
- **Python Paddle adapter**：`workers/argusflow-vision-worker/src/argusflow_vision_worker/worker.py`
- **WGC**：`crates/argusflow-windows/src/capture/wgc.rs`
- **Topology**：`crates/argusflow-windows/src/capture/topology.rs`
- **SendInput**：`crates/argusflow-windows/src/input/backend.rs`、`mouse.rs`
- **Visual resolver**：`crates/argusflow-windows/src/input/visual_resolver.rs`
- **Tauri wiring**：`src-tauri/src/runtime.rs`
- **默认微信流程**：`src/features/workflow/model/defaultWorkflowTemplate.ts`
- **微信 helper**：`src/features/workflow/model/wechatTemplateParts.ts`
- **Flow Components**：`src/features/workflow/components/componentCatalog.ts`
- **Node Presets**：`src/features/workflow/nodes/nodePresetCatalog.ts`
- **Palette**：`src/components/workflow/palette/nodePaletteCatalog.ts`
## 39. 两周整改节奏
- **Day 1**：冻结新增 Vision feature；建立 P0 issues；先补 failing output/transport tests。
- **Day 2**：合并 PR-1 output contract；明确 GetValue capability。
- **Day 3**：实现 protocol v2 binary/shared-memory happy path。
- **Day 4**：完成 large ROI、crash、timeout protocol integration。
- **Day 5**：实现 RefreshPlan 与 query∩dirty。
- **Day 6**：补 refresh metrics 与 benchmark，确认不是“有 DirtyMap 但仍全图 OCR”。
- **Day 7**：拆 persisted/prepared target；删除 VisualResolved serde 路径。
- **Day 8**：实现 Planner-owned materialization candidate chain。
- **Day 9**：Explain/Evidence 接入 materializer stage；SendInput 纯 actuator。
- **Day 10**：Canonicalize WeChat Component；默认模板移除复制图。
- **Day 11**：实现 pre/post scene delta 与 OutcomeUnknown。
- **Day 12**：完成 simulated WeChat fixture E2E。
- **Day 13**：Per-scope VisionRuntime；A/B/A cache test。
- **Day 14**：处理 Scroll public surface、OwnedPopup 承诺、WGC staging reuse；同步 docs。
## 40. 最终优先级表
| ID | 等级 | 核心问题 | 立即动作 |
|---|---|---|---|
| F01 | P0 | VisionBackend 输出 key 与 Runtime 节点端口不一致 | 把输出 key 的定义收敛为单一 `ActionOutputContract` 或 `AutomationAction::output_shape()`。 |
| F02 | P0 | Vision GetValue 用 OCR 可见文本冒充控件 value | P0 直接让 Vision GetValue 返回 Unsupported。 |
| F03 | P0 | BGRA `Vec<u8>` 被编码成 JSON 数字数组，4MB frame 对正常 ROI 不够 | P0 改为 JSON header + raw binary body，或直接实现已有 SharedMemory variant。 |
| F04 | P0/P1 | DirtyMap 只驱动 cache invalidation，没有真正驱动 OCR refresh ROI | 新增 `RefreshPlan { CacheOnly / Partial(regions) / Full }`。 |
| F05 | P0 | Visual Click 在 SendInput 内隐藏调用 VisionRuntime，绕过 Router | 引入 planner-owned `PreparedTargetMaterializer`。 |
| F06 | P0 | runtime-only `VisualResolved` 被放进可 serde 的 persisted TargetLocator | `TargetLocator` 只保留 persisted semantic locator。 |
| F07 | P0 | 微信发送后的 exact-text 验证可把历史同文本误判成本次发送成功 | 发送前记录 `PreSendObservation`：scene/frame、matching hashes、bottom anchor。 |
| F08 | P1 | dynamic WindowSet/OwnedPopup 目前主要只是 generation invalidation，并非真实多 surface capture | P0 若只支持 primary surface，就明确写限制。 |
| F09 | P1 | VisionRuntime 是多 subscription + 单全局 cache/frame/noise 状态 | 改成 `HashMap<VisualScopeKey, ScopeState>`。 |
| F10 | P1 | 默认微信 Workflow 与官方发送微信群消息 Component 重复硬编码同一业务图 | 把 `createWechatMessageDefinition()` 作为唯一 graph source。 |
| F11 | P1 | Scroll 子系统约 1346 行但没有进入主执行链 | 没有真实 consumer 前改为 `pub(crate)` 或 feature gate。 |
| F12 | P1 | WorkerSupervisor 已实现，但真实 worker lifecycle ownership 留给外部部署层 | 推荐 Desktop owns worker：generate token -> spawn -> health -> bounded restart -> shutdown。 |
| F13 | P1 | WGC readback 每帧创建 D3D11 staging texture | 把 staging texture 放进 WindowFrameSubscription state。 |
| F14 | P1 | 局部 scene refresh 会重置全局 `stored_at`，可能人为延长未刷新区域 freshness | 给 node/region 记录 observed_frame/observed_at，或维护 region freshness map。 |
| F15 | P2 | Visual resolver 把 OCR/Capture/Protocol 错误都归因为 SendInput | 随 Planner materialization 重构一起删除该映射。 |
| F16 | P2 | Capture subscription 缓存 key 忽略 CapturePolicy | 如果 policy 应 runtime-global，就把它移出 per-call SceneRefreshPolicy。 |
| F17 | P2 | 视觉 bbox 到屏幕坐标映射缺少真实 Windows DPI/边框 fixture 证明 | 把坐标变换封成 Windows-only `SurfaceTransform`，记录输入/输出 coordinate space。 |
| F18 | P2 | Python deadline 是推理完成后检查，无法真正取消 Paddle predict | P0 明确：超时即视 worker unhealthy，并按 lifecycle 策略 kill/restart，避免假装 cooperative cancel。 |
| F19 | P2 | `UiPayloadV2` 同时承载 v2/v3，schema migration ownership 不够清晰 | 把内部结构改名 `CurrentUiPayload`，或显式 UiPayloadV2/UiPayloadV3 + migrate。 |
| F20 | P2 | 节点 Palette 展示多个 `kind: null` 的未来节点，UI 能力契约不够明确 | 未实现项默认隐藏，或显示显式 `即将支持` badge 且 disabled。 |
## 41. 最终判断
- 有离谱实现：最典型是 raw BGRA -> JSON decimal array -> 4MB framed JSON -> Python list[int]。
- 有确定功能 bug：Visual output key 与 Runtime port contract 不一致。
- 有语义造假：Vision GetValue 用 OCR 可见文本冒充控件 value。
- 有“实现一半却按完成宣称”的风险：DirtyMap 与 OwnedPopup 都存在这种迹象。
- 有重复实现：默认微信 graph 与官方 Flow Component graph 双份维护。
- 有职责重复：ActionRouter 已存在，SendInput 内又通过 resolver 隐藏做一次视觉路由。
- 有不合规持久化边界：VisualResolved runtime-only 但可 serde。
- 有非幂等验证漏洞：历史同文本可造成 false success。
- 有 scope 失控：主链未稳定前先公开约 1346 行 Scroll framework。
- 也有值得保留的高质量部分：强类型 region、ValueExpr、0/1/N、PID 校验、Preset/Component IA。
- 因此建议不是整包 revert，而是停止叠功能、拆小 PR 把主链 contract 修实。
- 真正的 merge gate 应是：六个 P0 关闭 + contract/integration tests + simulated default flow E2E。
## 42. GitHub 定位
- Repo：`https://github.com/SLOE-debug/argusflow`
- Commit 733：`https://github.com/SLOE-debug/argusflow/commit/7335750b62563f7b49cf605db626685ba5f177b0`
- Commit e7d：`https://github.com/SLOE-debug/argusflow/commit/e7d0169d98a3e8bd6ed0ef486095f95a5863a4c5`
## 43. 最终 Merge Gate（全部满足才继续扩 Vision）
- [ ] MG01 output contract test 覆盖 GetText/GetValue/Extract。
- [ ] MG02 Vision GetValue 明确 Unsupported。
- [ ] MG03 生产 pixel transport 不使用 JSON byte array。
- [ ] MG04 large ROI protocol test 通过。
- [ ] MG05 worker token/version/request_id correlation test 通过。
- [ ] MG06 DirtyMap 可生成 CacheOnly/Partial/Full。
- [ ] MG07 query region 与 dirty region 正确求交。
- [ ] MG08 OCR pixel metrics 可观测。
- [ ] MG09 partial freshness 不污染其它区域。
- [ ] MG10 VisualResolved 从 persisted enum 移除。
- [ ] MG11 prepared target 不实现持久化 serde。
- [ ] MG12 Visual Click 的 materializer stage 由 Planner 管理。
- [ ] MG13 VisualCache hit 可以零 OCR 点击。
- [ ] MG14 Tiny success 可以零 Medium 点击。
- [ ] MG15 Medium 升级原因出现在 Explain。
- [ ] MG16 Vision root error 不冒充 SendInput。
- [ ] MG17 SendInput 只负责物理 actuation。
- [ ] MG18 materialized target 带 scene/frame/generation。
- [ ] MG19 actuation 前复验 window identity。
- [ ] MG20 历史同 message 回归测试通过。
- [ ] MG21 send Unknown outcome 不自动 retry。
- [ ] MG22 default WeChat graph 与 Component 单一来源。
- [ ] MG23 A/B/A window cache isolation 通过。
- [ ] MG24 stale subscription 可 prune。
- [ ] MG25 CapturePolicy caching 语义被测试。
- [ ] MG26 OwnedPopup 支持范围写进 docs 且与实现一致。
- [ ] MG27 WGC staging steady-state 不每帧创建。
- [ ] MG28 mixed DPI click fixture 通过。
- [ ] MG29 Scroll 无 consumer 时不作为稳定 public API。
- [ ] MG30 worker lifecycle owner 明确。
- [ ] MG31 PR CI 至少覆盖 Rust/TS contract tests。
- [ ] MG32 protocol integration 在 CI 可自动跑。
- [ ] MG33 Windows fixture 至少进入专用 CI/nightly。
- [ ] MG34 simulated WeChat E2E 覆盖 delayed render/duplicate text/failure。
- [ ] MG35 docs 中“已完成”只描述有测试证据的能力。
- [ ] MG36 下一项 Vision feature 以独立 vertical-slice PR 提交。
满足以上 Gate 后，再继续 Scroll、真正多 surface OwnedPopup、Grounding 与更多 Studio 资产。
---
（审计结束）
