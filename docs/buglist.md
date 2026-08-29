我看了最新 `main`，当前 HEAD 是 `2e995a8`，提交说明本身就是针对上次审计做整改。整体方向明显比上一版靠谱，但**还不建议把这版标成“审计通过”**：上次几个大坑基本修掉了，不过我仍然看到 **5 个需要阻断合并/发布的 P0，以及若干 P1**。

先说修好的部分：UI 输出现在收敛到了 Core 的统一 `ActionOutputContract`，`GetText / GetValue / Extract One / Extract Many` 的字段分别固定为 `text / value / item / items`，Runtime 和 Vision backend 都会做契约校验；Visual `GetValue` 也不再错误地返回 OCR 文本。   OCR 像素也已经从 JSON `Vec<u8>` 改成 `AFV2 JSON control + raw binary body`，之前 4MB JSON 爆帧的问题算真正修了。  此外 SendInput 不再持有 VisionRuntime，视觉物化提升到了 Planner 这一层，微信默认模板也已经从 canonical component 派生，不再维护两套完整流程。

剩下的问题按优先级看：

1. **P0：当前 HEAD 很可能直接 Rust 编译不过。** `UiPostcondition` 写成了 `NewText { pub query: VisualQueryExpr }`。Rust 的 enum variant field 不能写 `pub` 可见性修饰符，这不是风格问题，是语法/编译错误；应该直接写 `query: VisualQueryExpr`。更麻烦的是当前 commit 没有任何 GitHub status/check，状态接口是 `pending + total_count=0`，也没有 workflow run，所以没有 CI 帮忙挡住这种错误。

2. **P0：Visual Click 仍然绕过了用户的 `BackendPolicy`。** 这次虽然把物化移动到了 Planner，但 `VisualMaterializationPlan::for_input()` 仍然硬编码 `Cache -> OcrTiny -> OcrMedium`；Router 只检查了 `SendInput` 是否允许，随后就执行整条固定视觉链。也就是说用户如果明确 `deny: [ocr_medium]`，甚至只允许 `visual_cache`，Planner 仍可能偷偷跑 medium。这和“Planner 统一控制 backend policy”的设计目标是冲突的。  Runtime validation 明明还会检查 visual backend policy，因此这里形成了“校验说尊重 policy，真实执行却不完全尊重”的假契约。
   应该让 `VisualMaterializationPlan` 从 `PreparedAutomationTarget.backend_policy()` + runtime availability 构造，而不是 `for_input()` 固定生成。

3. **P0：`MaterializedTarget.topology_generation` 被记录了，却根本没有在点击前复验。** 新类型的注释自己明确写了：“捕获时的 topology generation，**输入前必须再次确认没有变化**”。但 `SendInputPreparedExecution::VisualClick` 只检查窗口 identity、safe point 是否在 bbox，再直接 `inject_click()`；`scene_id / frame_id / topology_generation` 三个 freshness 字段都没有参与最终输入校验。
   典型竞态是：
   `OCR 找到“确定” -> popup/窗口布局变化 -> 原位置变成“删除” -> SendInput 仍点击旧坐标`。
   既然已经专门增加 generation，就必须在最后一个物理输入 commit point 前用当前 topology 再比较一次；不一致就丢弃 materialized target，重新 materialize，而不是点击。

4. **P0：发送消息的“新增文本验证”目前还不足以证明消息是这次发送产生的。** 这里有三个子问题。动作前 baseline 使用的是 `SceneRefreshPolicy::tiny()`，它 `force_refresh=false` 且允许 500ms cache hit，所以 baseline 有可能不是 Enter 前那一刻，而是更早的旧 scene。  动作后则只做**一次** medium scene 获取和一次验证，没有 postcondition wait budget；微信渲染慢几十到几百毫秒，就会直接变成 `OutcomeUnknown`，不会再观察。  更关键的是“是不是新增”的判断用 `stable_hash + normalized_text`，而 `stable_hash` 包含 bbox 坐标；历史里原本就存在同样一句消息，只要因为新消息/滚动导致旧消息位置发生位移，它的 hash 就变了，就可能被判成一个新的节点。
   这个问题直接关系到非幂等“发送消息”，我会卡 P0。正确方向至少是：**baseline 强制 fresh；postcondition 有独立 observe deadline；跨 scene identity 不能把 bbox 当 identity 的组成部分**，应该做 text + 几何容差/row tracking/patch association。

5. **P0：OCR worker 的 timeout/restart 闭环还没真正落地。** Python worker 只预热了 tiny 的中英文模型，medium 仍然是第一次请求时 lazy load；但 Rust 给 OCR request 的 deadline 直接用了 `policy.stability.timeout`，默认只有约 800ms。于是第一次关键 medium 验证很可能把“加载 34M 模型 + 推理”一起塞进 800ms。  一旦超时，Python 代码会把 worker 标记 failed 并 `return` 退出；Rust 侧也把 engine health 标成 Failed。  README 现在说“deployment layer 负责 bounded restart”，但当前 Tauri 装配只在启动时做**一次** `refresh_health()`，仓库里也没有看到实际 supervisor/restart/re-handshake 闭环。  如果外部确实另有部署进程负责拉起 worker，那至少要补 integration test 证明 `worker dies -> restart -> same desktop app reconnect -> health Ready -> OCR resumes`；否则现在很容易一次 medium timeout 后整个运行期 Vision 永久挂掉。

6. **P1：Tiny 的 confidence 根本没有参与升级决策。** Materializer 的逻辑是 Cache 找到 unique 就返回，Tiny 找到 unique 也直接返回；只有 `TargetNotFound` 才继续到 Medium。`MaterializedTarget` 虽然保存 `confidence`，但没有最低置信度/关键动作阈值。因此一个 `0.41` 的 Tiny OCR 唯一结果照样可以被鼠标点下去。 这与原来的视觉设计“低置信度 -> medium”不一致。对于 Click，尤其高风险按钮，我建议做 `confidence + uniqueness + region` 联合 gate。

7. **P1：Visual Click 的 Cache stage 仍可能使用旧坐标。** `tiny()` cache 最长允许 500ms，而 dirty invalidation 只有下一次真正 capture 时才知道发生了变化；纯 `lookup_cache()` 本身并不会取得新 frame。所以“cache 还没过期”不等于“屏幕没有变”。 对读取操作，这种 cache-first 可以接受；对物理 Click，建议至少先做一次 cheap new-frame diff，再决定是否复用 bbox，或者直接把 actuation 的 cache freshness 收紧到极低范围。

8. **P1：`target_wait.timeout_ms` 现在不是严格总 deadline。** Router 是先算 deadline，然后 `await materializer.materialize(...)`；但没有用 `timeout_at(deadline, materialize())` 包住单次 materialization。一次 Tiny/Medium OCR 本身就可能超出目标等待总预算，多个 stage 更明显。因此配置“等 5 秒”理论上仍可能跑明显超过 5 秒。 这违背 docs 里“一节点一个总 deadline”的原则。

9. **P1：官方微信 Component 改了执行语义，却没有 bump version。** 旧版 `发送微信群消息` 是同一个 component ID、`1.0.0`；新版 `1.0.0` 给 `send_message` 增加了 `NewText` postcondition，语义已经显著改变，但 version 仍然保持 `1.0.0`。 之前 docs 明确要求 component instance 精确 pin 版本，旧工作流不能因为 catalog 更新而静默改变行为。现在相当于违背了自己刚建立的可审计规则。至少应该变成 `1.1.0`，如果认为失败语义属于 breaking change，我更倾向 `2.0.0`。

10. **P1/P2：一些“整改痕迹”还没完全收干净。** `send_message` 已经执行一次“新增消息验证”，后面又接了一个 `verify_message` GetText，等于同一个动作连续做两次视觉验证；后一个主要只是为了输出 `confirmed`，但会额外触发 OCR 和新的失败面。 Windows topology 现在也明确承认 `PrimaryOnly`：Owned Popup 虽然枚举出来，但不进入捕获 surface，所以微信如果某个搜索面板/菜单/对话框真的是独立 HWND，Vision 依然看不到它。 另外默认模板虽然已经从 canonical component 生成，但转换层用了多处 `as unknown as`，并通过“递归替换所有等于 `wechat_application` 的字符串”来改资源引用；以后如果普通文本字段刚好出现这个字符串，也会被误改，最好做 schema-aware ref rewrite。

### 我会怎么判这次提交

**上次的架构性问题大约已经解决了一大半，方向是正确的；这次主要不是“乱写一堆重复代码”，而是“整改后的安全闭环还有洞”。**

如果是我做 merge gate，我会要求下一次至少先解决这 5 件事：

`修 Rust 编译错误`
→ `BackendPolicy 真正约束视觉物化链`
→ `输入前复验 topology generation`
→ `重做 fresh baseline + tolerant identity + postcondition wait`
→ `把 worker restart/reconnect + medium model deadline 做成真实闭环`

然后补上 **`cargo check/test + frontend typecheck/test + Python protocol test + Windows Vision smoke E2E`**。当前 HEAD 没有任何 CI check，这一点本身就不应该继续容忍；像这次 enum field 的 `pub` 这种错误，本来应该在提交后几十秒内自动挡住，而不是人工审计才能发现。

如果只问“这次实习生有没有又写得很离谱”：**没有上次那么离谱，分层已经明显收敛；但非幂等发送验证、stale visual target、worker 生命周期这三块还属于会在真实机器上出事故的实现，不是小修小补。**
