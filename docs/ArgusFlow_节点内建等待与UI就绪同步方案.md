# ArgusFlow 节点内建等待 / UI 就绪同步方案
> 仓库：`SLOE-debug/argusflow`
> 基线：`main @ 9a1ae74e07ed10497ec9f3640a33d1cc6645ca51`
> 目标：把“为了让当前节点能够执行而等待 UI/DOM/UIA 目标出现”的能力收回节点内部，不再用独立 Delay 节点猜渲染时间。
> 约束：保留现有 AQL、ActionRouter / PreparedPlan、UIA / CDP、Failure Evidence 分层，不新增第二套 selector / wait DSL。

## 0. 结论
我赞成这个方向，并建议把它定成 ArgusFlow 的正式规则：
> **如果等待的目的，是满足当前节点的执行前置条件，那么等待属于当前节点，而不是工作流控制流。**
百度热搜当前是：
```text
打开 Chrome -> 等待百度热搜渲染(1500ms) -> 批量获取热搜标题和链接 -> 写文件
```
应改为：
```text
打开 Chrome -> 批量获取热搜标题和链接 -> 写文件
                         │
                         └─ 内建：自动等待自己的 target，出现即执行，超时则当前节点失败
```
最终用户只配置“目标等待：开启；超时：5000ms”，不再插“等待百度热搜渲染”节点。
---

## 1. 当前实现的核心问题
`src/features/workflow/defaultWorkflowTemplate.ts` 明确存在 `wait_baidu_ready_1`，类型是 `delay`、1500ms，位于 Browser 和 CollectLinks 之间。
`crates/argusflow-runtime/src/builtin_nodes/utility.rs` 中 Delay 最终只是：
```rust
tokio::time::sleep(Duration::from_millis(self.milliseconds)).await;
```
因此它不知道在等哪个 DOM/UIA 元素，也不知道目标是否已经提前出现或永远不会出现；它只是暂停，不是同步。
更关键的是，backend 已经自己在等：CDP executor 当前硬编码 `5s deadline + 100ms polling`；UIA 也有 `TargetWaitPolicy`，默认约 `2s + 75ms polling`。因此现在实际上存在“画布 Delay + backend 隐藏等待”两层甚至三层时序，CDP/UIA 行为还不一致。
这正是应该统一掉的地方。
---

## 2. 必须区分两类“等待”
**Readiness Wait**：等按钮出现、等输入框出现、等 DOM 列表渲染、等窗口可用。这是节点能否执行的一部分，应归属当前业务节点。
**Deliberate Delay**：限流明确要求停 2 秒、演示流程故意暂停、业务协议固定退避。这才是真正的 Delay/Pause。
所以 `argus.delay` 可以为了旧工作流兼容暂时保留，但不再作为 UI readiness 的推荐方案；前端建议改名“固定暂停”，放到高级节点区。
---

## 3. “等待放在各自节点上”应该怎样落地
不要给通用 `WorkflowNodeContract` 增加一个万能 `wait` 字段，因为不同节点等待的是不同事实。
建议保持节点自治：
| 节点 | 自己负责的时序语义 |
|---|---|
| Application | 已有 `launch_timeout_ms`，等待进程/窗口 |
| Browser | 已有 `launch_timeout_ms`，等待 CDP session/page target |
| Command | 已有 `timeout_ms`，等待进程结束 |
| UI | 新增 `target_wait`，等待自己的 AutomationTarget |
| Delay | 只表示真实固定暂停 |
这比做一个通用 Wait Node 更符合当前 Runtime 边界。
---

## 4. UI 节点契约：只升 `argus.ui` payload 版本
当前 Workflow 已经是 schema v8，而节点有独立 `type_id + version + payload`。因此这次不需要整体升 workflow schema，只需要把 `argus.ui` 从 v1 升到 v2。
建议 payload：
```json
{
  "operation": {
    "type": "collect_links",
    "target": { "...": "..." }
  },
  "execution": {
    "target_wait": {
      "mode": "bounded",
      "timeout_ms": 5000,
      "poll_interval_ms": 100
    }
  }
}
```
Rust 结构：
```rust
pub struct UiPayloadV2 {
    pub operation: UiOperation,
    pub execution: UiExecutionPolicy,
}
pub struct UiExecutionPolicy {
    pub target_wait: TargetWaitPolicy,
}
pub struct TargetWaitPolicy {
    pub mode: TargetWaitMode, // None | Bounded
    pub timeout_ms: u64,
    pub poll_interval_ms: u64,
}
```
`target_wait` 不要放进 `AutomationTarget`。Target 回答“我要操作谁”，ExecutionPolicy 回答“为了完成这个节点，我允许等多久”，职责不同。
---

## 5. 不要让用户重复写一份“等待 selector”
百度 CollectLinks 节点本来就已经有：
```text
css("#hotsearch-content-wrapper a.title-content .title-content-title")
```
那么等待条件应直接从这个 operation/target 派生：
```text
CollectLinks(target = X)
≈ wait until X can satisfy CollectLinks
  then CollectLinks(X)
```
不要设计成“等待 selector=X + 采集 selector=X”，否则 selector 会保存两份并产生漂移。
P0 不需要新建 Wait DSL，也不需要单独的“等待条件编辑器”。
---

## 6. “目标 ready”必须按动作语义判断
不能简单定义成“Element Exists”。
建议语义：
```text
Click:
0 个可执行目标 -> TargetNotFound，可等待
1 个目标       -> ready，执行
>1 个目标      -> AmbiguousTarget，立即失败
SetValue:
0 个目标       -> 可等待
有目标但不可写 -> ActionUnsupported，立即失败
唯一且可写     -> ready
GetText/GetValue:
0 个目标       -> 可等待
唯一可读目标   -> ready
CollectLinks:
0 个匹配链接   -> 可等待
>=1 个链接     -> ready，一次性批量采集
```
因此**只有 `TargetNotFound` 才进入 target wait**。Ambiguous、ActionUnsupported、非法 AQL 等配置错误不能靠等待掩盖。
---

## 7. 等待应统一放到 PreparedPlan，而不是继续散落 backend
推荐最终执行链：
```text
UiNode
  ↓
ActionDispatcher
  ↓
PreparedPlan       ← 统一 target wait / deadline
  ↓
PreparedCandidate
  ↓
UIA / CDP executor ← 单次 materialize / execute
```
UiNode 给 Dispatcher 传执行选项：
```rust
pub struct ActionExecutionOptions {
    pub target_wait: TargetWaitPolicy,
}
```
不要把 wait 塞进 `AutomationAction`，因为 AutomationAction 是动作语义，不是节点执行预算。
---

## 8. PreparedPlan 必须“prepare 一次，重复同一冻结计划”
现有 Selector Resilience 文档已经给出正确原则：
> **wait 是同一 PreparedPlan 的重复 materialize，不重新 parse、不重新 plan。**
建议把当前 `PreparedPlan::execute(self)` 重构成“单次尝试 + 带 deadline 的外层循环”：
```rust
async fn execute_with_wait(&self, policy: TargetWaitPolicy)
    -> Result<ActionOutcome, AutomationError>
{
    let deadline = Instant::now() + policy.timeout();
    loop {
        match self.execute_once().await {
            Ok(outcome) => return Ok(outcome),
            Err(AutomationError::TargetNotFound { .. })
                if Instant::now() < deadline =>
            {
                sleep(policy.poll_interval()).await;
            }
            Err(AutomationError::TargetNotFound { query }) => {
                return Err(AutomationError::TargetWaitTimeout {
                    query,
                    timeout_ms: policy.timeout_ms(),
                });
            }
            Err(error) => return Err(error),
        }
    }
}
```
每轮仍使用原来的 PreparedCandidate、branch_path、backend plan；绝不重新解析 AQL，也不重新创建另一套 fallback。
---

## 9. AQL `any(...)` 与 timeout：必须共享一个总 deadline
不能做成：
```text
branch0 等 5s -> branch1 再等 5s -> branch2 再等 5s
```
否则节点配置 5 秒，实际可能跑 15 秒。
正确规则：
> **一个 UI 节点只有一个总 deadline。**
每一轮 `execute_once()` 按 PreparedPlan 既有的确定性 candidate/fallback 规则尝试；只有这一轮整个 plan 最终仍为 `TargetNotFound`，才进入下一次 poll。
因此如果 `any(strong_selector, fallback_selector)` 的 fallback 当前已经命中，第一轮就应该成功，而不是先白等 strong selector 五秒。
---

## 10. CDP 与 UIA 应回归“单次执行”
### CDP
删除 `crates/argusflow-browser/src/cdp/executor.rs` 当前硬编码的 `5s / 100ms` target-not-found loop。
CDP executor 每次只做一次：
```text
Runtime.evaluate -> ok / not_found / ambiguous / backend error
```
`not_found` 立即返回 `AutomationError::TargetNotFound`，是否继续等待由 PreparedPlan 决定。
### UIA
删除 UiaExecutor 当前针对 `TargetNotFound` 的 TargetWaitPolicy 循环。
但以下 backend 可靠性逻辑必须保留：stale element retry、HWND/process identity 校验、COM worker recovery、transaction timeout、UiaExecutionBudget、provider unavailable。
分界线很清楚：
```text
业务目标暂时不存在 -> PreparedPlan 等
UIA/CDP 自身连接、stale、provider 问题 -> backend 自己处理
```
---

## 11. “实时监测”P0 建议 bounded polling，事件只做优化
产品上可以叫“实时等待目标出现”，但底层 P0 不建议把正确性强绑定到 DOM MutationObserver 或 UIA StructureChangedEvent，因为真实 UIA provider 的事件质量并不统一。
P0 建议：
```text
timeout = 5000ms
poll = 100ms
```
目标 120ms 出现，就大约 200ms 继续；目标 3.2s 出现，就大约 3.3s 继续；目标一直不出现，5s 后明确失败。
P1 可以做 event-assisted polling：
```text
poll timer OR DOM/UIA event wakeup
```
事件只负责提前唤醒，poll + deadline 仍是最终正确性保障。
---

## 12. timeout 应成为正式错误
当前 `AutomationError` 已有 `TargetNotFound / AmbiguousTarget / ActionUnsupported / BackendUnavailable / BackendFailed`。
建议新增：
```rust
TargetWaitTimeout {
    query: String,
    timeout_ms: u64,
}
```
语义：
```text
TargetNotFound = 单次 materialize 没找到
TargetWaitTimeout = 节点允许的整体等待预算耗尽
```
最终 UI 错误应类似：
```text
批量获取热搜标题和链接执行失败：
在 5000ms 内未等到目标：
css("#hotsearch-content-wrapper a.title-content .title-content-title")
```
这正是你说的“在指定时间内没等到 xxx 元素”。
---

## 13. 错误分类规则
只等待 `TargetNotFound`。
以下立即失败：`AmbiguousTarget`、`ActionUnsupported`、`BackendFailed`、非法 AQL、权限错误、表达式错误。
`BackendUnavailable` 继续沿用 PreparedPlan 当前已有 backend fallback，不要把 backend 挂了误判成“目标还没出现”。
原则：
> **等待只能恢复“目标暂时不存在”，不能掩盖“配置本来就是错的”。**
---

## 14. Failure Evidence 应在最终 deadline 后采集
第一次 miss 不要马上 dump DOM/UIA tree，因为那可能只是正常加载。
推荐：
```text
TargetNotFound -> poll -> TargetNotFound -> ... -> deadline
                                              ↓
                                      TargetWaitTimeout
                                              ↓
                                       capture evidence
```
建议补 `EvidenceTrigger::Timeout`。
最终 Evidence 至少带：`node_id / operation / normalized query / timeout_ms / elapsed_ms / attempts / backend / branch_path / selector trace / DOM-AX-UIA snapshot / screenshot`。
这能把“为什么超时”变成可诊断事实，而不是只知道 sleep 结束了。
---

## 15. 前端交互
在 `ActionNodeFields.tsx` 的 UI 节点高级设置中增加：
```text
目标等待
[x] 自动等待目标就绪
超时时间
[5000] ms
```
`poll_interval_ms` 不建议默认暴露，可放二级高级设置。
节点运行时仍然是同一个节点：
```text
● 批量获取热搜标题和链接
  等待目标… 1.2s / 5.0s
```
可增加三个轻量 ExecutionEvent：`target_wait_started`、`target_wait_satisfied`、`target_wait_timed_out`。它们只负责可观察性，不进入控制流。
---

## 16. 百度热搜模板具体改法
删除：
```text
wait_baidu_ready_1
edge_browser_wait
edge_wait_collect
```
新增：
```text
edge_browser_collect
```
CollectLinks 节点改成：
```ts
{
  id: 'collect_baidu_news_1',
  kind: 'ui',
  data: {
    kind: 'ui',
    label: '批量获取热搜标题和链接',
    operation: {
      type: 'collect_links',
      target: createBaiduCdpTarget(
        'css("#hotsearch-content-wrapper a.title-content .title-content-title")',
      ),
    },
    execution: {
      target_wait: {
        mode: 'bounded',
        timeout_ms: 5_000,
        poll_interval_ms: 100,
      },
    },
  },
}
```
最终画布只剩：
```text
开始 -> 打开 Chrome -> 批量获取热搜标题和链接 -> 写入桌面 -> 输出路径 -> 结束
```
“等待百度热搜渲染”没有消失，而是成为 CollectLinks 节点内部的 readiness policy。
---

## 17. `argus.delay` 兼容策略
不建议立刻删除 `argus.delay v1`，否则旧 workflow 会坏。
建议 P0：默认模板不再使用 Delay；UI 节点支持 target_wait；文档不再推荐 Delay 解决 UI readiness。
P1：前端把“等待”改名“固定暂停”，移到“高级/控制”，提示“仅用于主动暂停，不应用于等待 UI/DOM 元素出现”。
不要自动把所有 `Delay -> UI` 转成 target_wait，因为“固定暂停 2 秒”不一定等价于“最多等目标 2 秒”。旧工作流保持原语义即可。
---

## 18. 版本策略
这次无需 workflow schema v9。
当前节点 envelope 已经有独立：
```text
type_id
version
payload
```
因此：
```text
argus.ui v1 = { operation }
argus.ui v2 = { operation, execution }
```
新建/编辑后的 UI 节点统一保存 v2；Runtime 可继续支持 v1，避免旧文件失效。
---

## 19. 默认值建议
P0：
```text
Query locator:
  mode = bounded
  timeout_ms = 5000
  poll_interval_ms = 100
```
Coordinate：`mode = none`，因为坐标没有“元素出现”语义。
Visual/OCR 如果进入统一等待，建议 backend 对实际采样设置更高的最小间隔（如 300ms），避免每 100ms 做昂贵视觉推理；但总 timeout 仍由节点统一控制。
---

## 20. 主要修改文件
```text
src/features/workflow/contracts.ts
src/features/workflow/workflowModel.ts
src/features/workflow/workflowNodeDefinitions.ts
src/features/workflow/defaultWorkflowTemplate.ts
src/components/workflow/ActionNodeFields.tsx
src/components/workflow/NodeInspectorFields.tsx
src/components/workflow/NodePalette.tsx
crates/argusflow-core/src/error.rs
crates/argusflow-runtime/src/builtin_nodes/ui.rs
crates/argusflow-agent/src/plan.rs
crates/argusflow-agent/src/router.rs
crates/argusflow-browser/src/cdp/executor.rs
crates/argusflow-windows/src/uia/executor.rs
crates/argusflow-windows/src/uia/plan.rs
crates/argusflow-windows/src/uia/runtime.rs
```
---

## 21. 必须覆盖的测试
1. 目标 300ms 后出现、timeout=5s：约 300~500ms 继续，不能固定睡满。
2. 目标永不出现：约 5s 后 `TargetWaitTimeout`，包含 query/timeout。
3. 第一次就 Ambiguous：立即失败，不 retry 5s。
4. 元素存在但 ActionUnsupported：立即失败。
5. `any(branch0, branch1)`：branch0 不存在但 branch1 已存在时第一轮成功，timeout 不能相乘。
6. UIA unavailable、其他 backend ready：沿现有 backend fallback，不进入 target wait。
7. 普通 miss 不落最终失败 Evidence；deadline 后采集。
8. 默认百度模板不再存在 `wait_baidu_ready_1`。
9. 旧 `argus.delay v1` 仍可加载和执行。
10. CDP/UIA executor 不再拥有业务级 TargetNotFound 长轮询。
---

## 22. 推荐实施顺序
```text
1. 增加 UiExecutionPolicy / TargetWaitPolicy / argus.ui v2
2. UiNode -> ActionDispatcher 传 ActionExecutionOptions
3. PreparedPlan 拆 execute_once / execute_with_wait
4. 新增 TargetWaitTimeout + EvidenceTrigger::Timeout
5. 删除 CDP 硬编码 5s/100ms target wait
6. 移除 UIA TargetNotFound wait loop，保留 stale/provider recovery
7. 改百度默认模板，删除独立 Delay
8. 前端把 Delay 改名“固定暂停”并移到高级区
9. 补 target_wait_* 运行事件和进度 UI
```
---

## 23. 最终模型
```text
UI Node
├─ operation
│  └─ target
│     ├─ scope
│     ├─ AQL / visual / coordinate
│     └─ backend policy
└─ execution
   └─ target_wait
      ├─ timeout
      └─ poll interval
```
运行时：
```text
Node starts
  ↓
Prepare 一次
  ↓
PreparedPlan.execute_once()
  ├─ Success -> Node success
  ├─ TargetNotFound
  │    ├─ deadline 未到 -> poll/event wakeup -> 同一 PreparedPlan 再试
  │    └─ deadline 到 -> TargetWaitTimeout -> Evidence -> Node failed
  └─ Other Error -> 沿现有 fallback / failure 语义
```

## 24. 一句话定性
> **Wait Node 是控制流；“等待当前节点依赖的 UI 目标就绪”是节点执行前置条件。**
对于 ArgusFlow，后者应该收回 UI / Application / Browser 等节点内部，并由 PreparedPlan 统一提供 bounded wait + timeout。
百度热搜最终就应该是：
```text
打开 Chrome -> 批量获取热搜标题和链接 -> 写入文件
```
其中 CollectLinks 内建：“最多等待自己的 target 5 秒；出现立即继续；超时明确报在指定时间内未等到 xxx 目标，并保留失败现场。”
这比独立 Delay 更快、更稳、更容易诊断，也更符合当前 AQL / PreparedPlan / Failure Evidence 架构。
---

## 参考实现与设计文档
```text
docs/ArgusFlow_Selector_Resilience_Failure_Evidence_Design.md
docs/ArgusFlow_AQL_统一UI查询语言设计方案.md
docs/ArgusFlow_App_Run_Node_Design.md
docs/ArgusFlow_变量与流程运行时设计方案.md
src/features/workflow/defaultWorkflowTemplate.ts
src/features/workflow/contracts.ts
src/features/workflow/workflowNodeDefinitions.ts
src/components/workflow/ActionNodeFields.tsx
crates/argusflow-runtime/src/builtin_nodes/utility.rs
crates/argusflow-runtime/src/builtin_nodes/ui.rs
crates/argusflow-agent/src/plan.rs
crates/argusflow-browser/src/cdp/executor.rs
crates/argusflow-windows/src/uia/executor.rs
crates/argusflow-windows/src/uia/plan.rs
crates/argusflow-core/src/error.rs
```
