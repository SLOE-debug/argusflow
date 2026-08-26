# ArgusFlow 最新提交架构与性能深度审查报告

> 审查仓库：`SLOE-debug/argusflow`
> 审查分支：`main`
> 审查基线：`79e00a392c86af84d1c1c621174eec5c9f514db7`
> 提交时间：2026-08-26 13:52:12 UTC
> 提交标题：`feat: 实现 CDP 浏览器工作流与百度热搜采集`
> 审查重点：Browser/CDP、Windows UIA、Planner/Runtime、性能与并发、低耦合高内聚、扩展性，以及 ECS-inspired 重构方向。

> **实现状态（2026-08-27）：** 本文保留 `79e00a3` 基线判断作为设计依据；已经完成的项目会在对应章节标记。当前工作树已落地 schema v7、开放节点/资源/能力契约、强类型 prepare、多个 RunWorld 和资源访问调度，不应再把这些基线问题当作现状。

---

## 0. 结论先行

我的总体评价是：

**ArgusFlow 现在已经有一套“不错的自动化后端内核”，但还没有完全形成一套“开放式、高吞吐、数据驱动的自动化运行时”。**

最值得肯定的是，当前项目已经把很多很难后补的基础边界做对了：

- `argusflow-core / query / agent / runtime / browser / windows / vision` 的 crate 边界总体合理；
- `ActionBackend -> PreparedCandidate -> PreparedExecution` 这一条链路很干净，已经接近插件式系统；
- `ResourceId / ResourceRef / AppSession / BrowserSession` 把持久化定义和真实 OS/COM/WebSocket 句柄分离，这是非常重要的设计；
- UIA 没有直接把 COM 对象扔进 Tokio，而是放进专用 MTA worker，并做了 generation-aware recovery；
- CDP 使用一条持久 WebSocket，通过 request id + pending map 做多路复用，而不是每个动作重新建连；
- 查询编译器已经有 alternative expansion budget、support/cost/branch path、Explain 等概念，说明项目不是“动作脚本堆砌”，而是在往执行引擎发展。

本文基线聚焦三个结构性问题；前两项仍有后续工作，第三项已在本轮解决：

1. **CDP 的真正 AX/DOM native semantic executor 仍未完成。**
   本轮已让 compiler / Explain 如实描述实际执行路径，但非 CSS 的语义查询仍进入页面 DOM 解释器，先 `querySelectorAll('*')`，再用自定义 `implicitRole / accessibleName` 过滤。
   这仍会影响：
   - 语义正确性；
   - 大 DOM 下的性能。

2. **UIA 的正确性和容错设计明显强于其吞吐设计。**
   当前一个 `UiaRuntime` 仍只有一个串行 MTA worker，所有 UIA 请求共享一个 FIFO。ProcessId/native equality pushdown、BuildCache 和 First/Nth early-stop 已完成，但 AppSession-affinity worker pool、bounded mailbox 和关系遍历批量缓存仍未完成。
   这对于鲁棒性很好，但多应用并发、WPF/WinUI/custom provider 场景仍会受队列与 IPC 吞吐限制。

3. **基线中的工作流节点/资源类型曾是封闭 enum 模式；本轮已经解决 Runtime 中央分发问题。**
   后端现使用 `NodeEnvelope -> NodeTypeRegistry -> PreparedNode`，资源实例冻结自己的类型、清理策略和访问键；新增后端节点不再修改 `WorkflowNodeKind`、统一 executor match 或 cleanup match。前端内置节点仍需提供自己的编辑定义和渲染模块，这是产品 UI 的显式装配边界，不再影响后端执行契约。

因此我不建议把整个项目推倒重做成传统游戏 ECS；更合适的是：

> **保留现有强类型 compiler + PreparedPlan，把 Definition/Registry 做开放，把 Runtime 做 ECS-inspired 的 RunWorld + Systems，并在 compile/prepare 阶段把动态扩展“冻结”为紧凑的执行计划。**

这样能同时保留 Rust 的类型安全、现在已经做好的 Planner，又能得到你喜欢的 ECS 那种灵活扩展和低运行时分派成本。

---

# 1. 最新提交做了什么

最新提交 `79e00a3` 的性质不是普通功能补丁，而是把 Browser/CDP 从“一个后端概念”推进成了完整资源生命周期：

```text
Workflow Browser Node
        │
        ▼
BrowserSessionProvider
        │
        ▼
CdpRuntime
 ├─ Chromium process
 ├─ isolated user-data-dir
 ├─ root CDP WebSocket
 ├─ Target.attachToTarget(flatten=true)
 ├─ CdpPageSession
 └─ CdpSessionRegistry
        │
        ▼
ActionRouter / CdpBackend
        │
        ▼
PreparedExecution
        │
        ▼
Runtime.evaluate / page interpreter
```

这次提交新增/扩展的关键部分包括：

- 隔离 Chromium 启动；
- `--remote-debugging-port=0` 随机调试端口；
- 每次资源独立 `user-data-dir`；
- `DevToolsActivePort` 发现；
- 根 WebSocket 持久连接；
- `Target.attachToTarget(flatten=true)`；
- Browser resource scope；
- `CdpSessionRegistry`；
- `CollectLinks` 批量输出；
- workflow/browser 前后端类型与校验；
- E2E 测试代码。

**方向是正确的。**

尤其是 Browser 被建模成“资源”，而不是“某个 Ui 节点偷偷启动 Chrome”，这是以后做 tab、context、下载、网络监听、Cookie、HAR、页面池等功能的必要前提。

---

# 2. 当前整体架构地图

当前可以粗略理解为：

```text
┌──────────────────────────────────────────────────────────────┐
│                         Definition                           │
│  WorkflowDefinition / Node / UiOperation / ResourceRef       │
└───────────────────────┬──────────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────────┐
│                      Query / Compiler                        │
│ AQL -> normalized AST -> backend-specific query plans        │
└───────────────────────┬──────────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────────┐
│                      Agent / Planner                         │
│ ActionBackend                                                │
│   -> PreparedCandidate                                       │
│   -> Explain / Support / Cost / ContextFitness              │
│   -> PreparedPlan                                            │
└───────────────┬───────────────────────┬──────────────────────┘
                │                       │
                ▼                       ▼
       ┌────────────────┐      ┌──────────────────┐
       │ Windows UIA    │      │ Browser CDP      │
       │ MTA worker     │      │ async WS actor   │
       │ COM provider   │      │ page session     │
       └────────────────┘      └──────────────────┘
                │                       │
                └───────────┬───────────┘
                            ▼
┌──────────────────────────────────────────────────────────────┐
│                      Workflow Runtime                        │
│ ResourceTable / RunContext / NodeExecutor / cleanup          │
└──────────────────────────────────────────────────────────────┘
```

这里最强的抽象层其实不是 Workflow，而是 **ActionBackend/PreparedPlan**。

这是一个非常重要的判断。

---

# 3. CDP：架构写得怎么样？

## 3.1 做得好的地方

### 3.1.1 持久 WebSocket + actor 是正确方向

`crates/argusflow-browser/src/cdp/protocol.rs`

当前 `CdpConnection`：

- 一次连接；
- `mpsc` 请求入口；
- 每次请求独立 oneshot；
- monotonically increasing request id；
- `HashMap<id, PendingRequest>`；
- 单 actor 拥有 writer/reader/pending；
- 不需要给 WebSocket 外面套一个巨大的 async mutex。

这是比下面这种实现强很多的：

```text
action
 -> connect websocket
 -> send
 -> wait
 -> close
```

也强于：

```text
Arc<Mutex<WebSocket>>
```

因为当前 actor 明确把协议所有权收敛到单任务里，调用侧只拿轻量 sender。

这一点在高并发 CDP 下是正确的基础。

---

### 3.1.2 `flatten=true` 是合理的 session 模型

当前：

```text
Target.attachToTarget(targetId, flatten=true)
```

之后请求通过顶层 CDP 消息的 `sessionId` 路由。

这是现代 CDP 推荐的 flat session 模式，未来多 tab / worker / iframe target 都更容易围绕这一模型继续扩展。

---

### 3.1.3 Browser 资源生命周期边界很好

`CdpRuntime` 没有把 `Child`、WebSocket、profile 路径塞进 `BrowserSession`。

公开的 `BrowserSession` 只包含：

- ResourceId
- BrowserSpec
- process_id
- target_id

真正不可序列化/不可复制的状态留在 runtime：

```text
CdpRuntime
 ├─ sessions: ResourceId -> Arc<CdpPageSession>
 └─ browsers: ResourceId -> ManagedBrowser
```

这是非常健康的“逻辑资源 / 物理资源”分层。

以后即使 BrowserSession 被放进 RunContext、事件或错误上下文，也不会把 socket/Child ownership 搞乱。

---

### 3.1.4 隔离策略不错

浏览器启动包含：

- `127.0.0.1`
- 随机 remote debugging port
- 隔离 user-data-dir
- no first run
- disable background networking
- cleanup profile

默认安全性和可重复性比直接 attach 用户日常 Chrome 好得多。

---

### 3.1.5 CSS + 批量数据抽取路线很高效

像：

```text
css("a.news")
 -> 一次 Runtime.evaluate
 -> 页面内完成 querySelectorAll
 -> 一次性返回 links[]
```

这类任务正是 CDP 比 UIA 天然强的地方。

相比 UIA：

```text
找到元素
读取文本
读取 href
逐个跨进程调用
...
```

页面内批处理后只返回一个紧凑 JSON，吞吐潜力非常高。

---

# 4. CDP 当前最大的 P0 问题：Planner 与真实 Executor 不一致

这个是本次审查最重要的发现。

## 4.1 Compiler 的抽象

`cdp/compiler.rs` 会生成：

```rust
CdpMatcherPlan {
    source: AccessibilityTree | Dom,
    role,
    pushdown,
    projected_attributes,
    residual,
}
```

并且 capability 会依据这些事实给出：

```text
Native + Low
Hybrid + Medium
Emulated + High
```

Explain 也会展示：

```text
Accessibility tree
N native predicate(s)
project [...]
N residual predicate(s)
```

从设计上看，这很好。

---

## 4.2 但 page executor 没有真正执行这个 plan

在 `page_script.rs` 中，`CdpMatcherPlan` 被转为页面 DTO 时：

```text
source                  -> 丢失
pushdown/residual       -> 合并
projected_attributes    -> 丢失
```

页面里的 Match 实际执行逻辑接近：

```javascript
document.querySelectorAll('*')
  .filter(roleMatches)
  .filter(predicateMatches)
```

也就是说：

### compiler 认为

```text
AccessibilityTree source
+ native name equality
+ Low cost
```

### executor 实际上做

```text
扫描整棵 DOM
+ JS 自己模拟 role
+ JS 自己模拟 accessible name
```

这是一条明显的 **planner-executor semantic gap**。

---

## 4.3 为什么这是严重问题

### A. 性能成本被低估

对非 CSS 的 Match，当前可能会被标成 `Low`，但执行是 O(N) DOM 扫描。

如果 DOM 50k 节点，代价与 CSS selector/native AX query 完全不是一个数量级。

---

### B. Explain 不再是“真实 Explain”

现在 Explain 可能告诉开发者：

```text
Accessibility tree
native predicate
```

但实际没有使用 `Accessibility.queryAXTree`。

这会影响整个 Planner 的可信任性。

对于一个强调 explainable planning 的项目，这是架构级问题，不只是代码小 bug。

---

### C. accessible name 语义不完全等价

页面脚本自己实现了：

- aria-labelledby
- aria-label
- labels
- alt/title
- innerText/textContent

但浏览器真正的 accessible name computation 比这个复杂得多。

Shadow DOM、ARIA 规则、隐藏节点、presentation、label precedence、特殊 native semantics 等场景都可能出现差异。

Chromium 已经提供：

```text
Accessibility.queryAXTree
```

它可以按 Chromium 实际计算的 accessible name / role 查询 AX subtree。

如果 ArgusFlow 在 Planner 中说它走 Accessibility Tree，就应当真的使用这个域。

---

## 4.4 P0 建议

我建议二选一，最好最终做方案 B。

### 方案 A：短期先“诚实降级”

在真正 AX executor 完成以前：

```text
非 CSS Match
=> SupportLevel::Emulated
=> QueryCost::Medium / High
```

Explain 写清楚：

```text
DOM full-tree semantic emulation
```

先保证 Planner 说的和实际做的一样。

---

### 方案 B：真正实现 executor specialization

把 executor 拆成：

```text
CdpPlanExpr
   │
   ├─ Css
   │   └─ DOM/querySelector fast path
   │
   ├─ Match(source=Dom)
   │   └─ DOM-native candidate narrowing
   │
   └─ Match(source=AccessibilityTree)
       └─ Accessibility.queryAXTree
```

AX path：

```text
Accessibility.queryAXTree
      │ role/name
      ▼
AXNode[]
      │ backendDOMNodeId
      ▼
DOM node / Runtime object
      │
      ▼
action adapter
```

如果动作需要 DOM element，可以从 AX node 关联到 backend DOM node，再 resolve 成 Runtime object。

这样 compiler 的抽象才真正成立。

---

# 5. CDP 性能：当前到底算不算高性能？

答案是：

> **底层连接架构具备高性能潜力；CSS/批量采集已经是高效路线；语义 AQL matcher 当前还不能称为真正的高性能实现。**

我会把它分成三层评价。

| 层 | 当前评价 |
|---|---|
| Transport | 好 |
| Session/runtime | 好 |
| Query execution | CSS 好，semantic matcher 有明显优化空间 |

---

## 5.1 Transport 当前优点

- persistent connection；
- async；
- multiplexed request ids；
- no socket lock around entire request；
- bounded input channel。

---

## 5.2 但“channel=128”并不等于“in-flight=128”

当前 channel 是有界的：

```rust
mpsc::channel(128)
```

但 actor 从 channel 收到后会马上：

```text
pending.insert(id, request)
writer.send(...)
```

只要 writer 能继续写，channel 就会被不断排空。

如果 Chromium 很久不返回，pending map 可以继续增长。

因此现在真正有界的是：

```text
waiting to enter actor
```

而不是：

```text
in-flight commands
```

### 建议

加独立 `Semaphore`：

```text
CdpConnection
 ├─ mailbox: 128
 └─ inflight permits: 32/64/etc
```

permit 生命周期从 command 提交到 response 完成。

这才是真正的协议级 backpressure。

---

## 5.3 cancellation / timeout 后 pending 不能立即删除

调用者 drop oneshot receiver 或外层 timeout 后：

```text
pending[id]
```

仍会存在，直到：

- Chromium 最后返回；
- WebSocket 断开；
- fail_pending。

如果网络/renderer 卡住，就会形成 pending retention。

### 建议

Command abstraction 增加：

```text
request_id
deadline
cancellation
```

actor 接收 CancelRequest：

```text
Cancel(id)
 -> pending.remove(id)
```

同时让 caller timeout 和 pending lifecycle 是同一个模型。

---

## 5.4 一个 actor 同时读写，writer backpressure 可能阻塞 read

当前 `writer.send().await` 在同一个 `select!` 分支里。

正常情况下没有问题。

但极端 backpressure 下，如果 send 长时间 pending，reader 分支不能同时推进，可能造成 head-of-line。

更极致的设计可以拆：

```text
request scheduler
      │
      ▼
writer task
      │
      websocket
      ▼
reader task
      │
      ▼
pending dispatcher
```

不过这不是当前第一优先级。

先解决：

1. in-flight cap；
2. cancellation；
3. event handling；

收益更大。

---

# 6. CDP 等待模型当前效率不高

`execute_cdp_action`：

```text
deadline = now + 5s

loop:
    Runtime.evaluate(full script)
    if not_found:
        sleep(100ms)
```

最坏大概会执行几十次完整页面脚本。

而页面脚本里的非 CSS matcher可能每次：

```text
querySelectorAll("*")
```

因此：

```text
5 秒 selector wait
× 100ms polling
× full DOM traversal
```

很容易成为热路径。

---

## 6.1 更好的等待模型

### DOM query

可以一次 evaluate 后，在页面内：

```text
MutationObserver
 + timeout
 + same evaluator
```

让一个 Promise 等元素出现。

这样只有一次 CDP round trip。

---

### Page/target 生命周期

CDP 本身有事件：

- `Page.lifecycleEvent`
- `Runtime.executionContextCreated`
- `Runtime.executionContextDestroyed`
- `Target.targetCreated`
- `Target.targetInfoChanged`
- `Target.detachedFromTarget`

当前 `protocol.rs` 明确把所有没有 `id` 的 event 直接忽略。

这会限制未来：

- navigation-aware wait；
- new tab；
- popup；
- SPA context reset；
- crash detection；
- target detached；
- network idle；
- download lifecycle。

---

## 6.2 推荐把 CdpConnection 升级为 Event Bus

目标：

```text
                       ┌── response(id) -> oneshot
WebSocket reader ──────┤
                       └── event(method/session) -> subscribers
```

例如：

```text
CdpEventHub
  ├─ session target lifecycle
  ├─ Runtime context lifecycle
  ├─ Page lifecycle
  └─ optional Network events
```

并让 `CdpPageSession` 自己维护：

```text
Healthy
Navigating
Detached
Crashed
Closed
```

这样 Router 的：

```rust
attached: true
```

就不再只是根据“Resource 还存在”推断，而是来自真正协议状态。

---

# 7. CDP 错误分类会阻断 Planner fallback

当前 CDP protocol transport 错误在 executor 中最终进入：

```text
AutomationError::BackendFailed
```

而 `PreparedPlan` 的 fallback 策略是：

- `BackendUnavailable`：可以 fallback；
- `TargetNotFound`：切下一 branch；
- 其它错误：终止。

因此：

```text
WebSocket disconnected
target detached
connection actor died
```

这种“后端当前不可用”的错误，可能被当成最终 BackendFailed，而不是让 Planner 尝试别的等价路径。

### 建议

把 CDP 错误至少分成：

```text
TransportUnavailable
SessionDetached
TargetCrashed
MethodRejected
InvalidExecutorResponse
PageScriptFailure
```

映射：

```text
TransportUnavailable
SessionDetached
TargetCrashed
    -> BackendUnavailable

MethodRejected
InvalidExecutorResponse
PageScriptFailure
    -> BackendFailed
```

这样会和现有 PreparedPlan fallback 语义更匹配。

---

# 8. CDP Browser 启动/清理：隔离好，但冷启动昂贵

当前每个 Browser resource：

```text
new process
new profile
new CDP endpoint
new page
```

这是非常好的 isolation default。

但如果未来目标是：

```text
100 个短工作流
每个只抓 1 个页面
```

Chromium cold start 会占主要成本。

---

## 8.1 推荐支持显式 reuse policy

不是把默认隔离取消，而是加可选：

```text
BrowserIsolationPolicy
 ├─ DedicatedProcess
 ├─ SharedProcessIsolatedContext
 └─ PooledContext
```

CDP 有 BrowserContext 概念。

可以：

```text
一个 Chromium
 ├─ BrowserContext A
 │   └─ page
 ├─ BrowserContext B
 │   └─ page
 └─ BrowserContext C
     └─ page
```

安全边界、Cookie/storage 隔离与冷启动吞吐之间可以由用户/节点策略决定。

---

## 8.2 Chromium 进程树所有权可以更强

目前：

```text
Browser.close
 -> wait 2s
 -> kill root child
```

通常够用。

但 Chromium 是多进程。

项目命令执行那边已经很强调 Windows Job Object 生命周期，那么 browser runtime 最终也可以考虑统一到：

```text
ManagedProcessTree
```

抽象里。

这样：

```text
Command
Browser
future driver/process resources
```

可以共享一个可靠的进程树 containment layer。

---

# 9. UIA：当前架构质量其实很不错

与很多自动化项目相比，这部分我评价较高。

## 9.1 COM thread ownership 做对了

`UiaRuntime` 没有让：

```text
IUIAutomationElement
IUIAutomation
```

跨 async task 到处传播。

而是：

```text
Tokio caller
   │ plain Rust request
   ▼
mpsc
   ▼
dedicated MTA worker
   │
   ├─ COM apartment
   ├─ CUIAutomation8
   └─ all COM objects stay here
```

这是正确的工程边界。

---

## 9.2 generation recovery 很成熟

它处理了一个 UIA 最麻烦的问题：

> 第三方 provider 卡死怎么办？

当前模型：

```text
generation 0
   │ timeout / dead worker
   ▼
generation 1
```

health 里同时编码 generation 和 state，旧 worker 退出不能覆盖新 worker 状态。

这是比较成熟的容错设计。

---

## 9.3 provider timeout 明确配置

已经调用：

- `SetConnectionTimeout`
- `SetTransactionTimeout`

并且 ArgusFlow 自己还有 execution deadline。

这是明显加分项。

---

## 9.4 遍历 budget 也做了

例如：

- max traversal nodes = 10,000
- max relation roots = 256
- bounded retry
- stale element retry

说明 UIA executor 对“provider/树可能恶化”的风险有意识。

---

# 10. UIA 最大性能瓶颈 1：单个全局串行 worker

当前：

```text
UiaRuntime
  └─ one UiaWorkerGeneration
       └─ std::mpsc FIFO
            ├─ Request A
            ├─ Request B
            ├─ Request C
            └─ ...
```

同一个 worker：

```text
while recv:
    execute synchronously
```

所以即使调用方是 20 个 async task：

```text
UIA effective parallelism ≈ 1
```

---

## 10.1 为什么当初这样写是合理的

这是非常保守、正确性优先的设计：

- COM ownership 明确；
- provider access 简单；
- mutation ordering 明确；
- recovery 简单；
- diagnostics 共享同一个 apartment。

所以我不会说这是“错误架构”。

它是一个很好的第一阶段。

---

## 10.2 但高吞吐阶段应该做“按资源分片”

不建议简单开 8 个 worker 随便抢请求。

更合理的是：

```text
UiaWorkerPool
  ├─ shard 0: process/app A
  ├─ shard 1: process/app B
  ├─ shard 2: process/app C
  └─ shard N
```

routing key：

```text
AppSession ResourceId
或 process_id
```

同一 AppSession：

```text
动作仍保持有序
```

不同应用：

```text
可以并行
```

这样既保留 UI 自动化的确定性，也解决全局串行瓶颈。

---

## 10.3 mailbox 也应该有界

UIA worker 当前使用标准：

```rust
std::sync::mpsc::channel()
```

这是无界队列。

在单 worker 速度低于 producer 时，会积累：

```text
requests
owned plans
oneshot senders
strings
```

虽然 timeout recovery 会让过期 generation 后续请求不再真正执行，但 burst memory 仍可增长。

建议：

```text
bounded sync_channel
flume bounded
crossbeam-channel bounded
```

加显式 overload error：

```text
BackendBusy / QueueSaturated
```

让 Planner/Engine 能做背压，而不是无限排队。

---

# 11. UIA 最大性能瓶颈 2：桌面根 Descendants 搜索只下推 ControlType

`process_search.rs` 当前是为了处理一个真实问题：

> Notepad++ 菜单、dialog 等 provider fragment 可能不在原 HWND 子树里。

因此它从：

```text
desktop root
```

搜索整个 UIA desktop，然后用 PID 再筛。

这个 correctness 目标是合理的。

但 discovery condition 当前基本只有：

```text
ControlType == expected
```

然后对每个候选：

```text
CurrentProcessId()
matches_current()
Current property...
```

这是明显的 IPC 热点。

微软官方也明确建议不要随意从 root 做 Descendants 搜索，因为可能遍历数百/数千元素。

---

# 12. UIA P0 优化：把 ProcessId 和稳定条件下推

native condition 应该优先成为：

```text
AND(
    ProcessId == target_pid,
    ControlType == target_type,
    safe native predicates...
)
```

也就是让 UIA provider/automation core 尽早缩小候选。

例如可以考虑原生下推：

- ProcessId
- ControlType
- AutomationId exact
- ClassName exact
- IsEnabled exact
- IsOffscreen exact
- 部分稳定 boolean

对 Name 是否完全 native pushdown可以继续保守，因为不同 provider 行为和大小写/本地化策略可能影响语义。

---

## 12.1 这会保留现有正确性优势

仍然是：

```text
desktop root
```

因此独立 fragment 还能找到。

但不再是：

```text
桌面所有 button
 -> 一个个 CurrentProcessId
```

而是尽可能让 native condition 直接筛成：

```text
这个 PID 的目标 ControlType
```

这是目前 UIA 最值得优先做的性能优化之一。

---

# 13. UIA 最大性能瓶颈 3：逐属性 Current 调用过多

`matches_current()` 会调用：

```text
CurrentIsControlElement
CurrentControlType
GetCurrentPropertyValue
GetCurrentPropertyValue
...
```

这些 UIA property access 在跨进程 provider 情况下并不便宜。

当前 cache 的使用方式是：

```text
候选已经找到
 -> BuildUpdatedCache(single element)
```

而且只对 residual 属性做。

这错过了 UIA 的一个重要优化能力：

> bulk fetching / caching。

微软文档明确指出：逐元素、逐属性访问会产生跨进程调用，缓存可以一次获取多个元素的多个属性。

---

# 14. UIA 推荐改成 Find*BuildCache

对于普通查询：

### 当前

```text
FindAll(condition)
    │
    ▼
element
  ├─ CurrentProcessId
  ├─ CurrentControlType
  ├─ CurrentName
  ├─ Current...
  └─ BuildUpdatedCache(residual)
```

### 推荐

```text
FindAllBuildCache(
    condition,
    cache_request(all required projections)
)
    │
    ▼
Cached elements
```

然后 local filter 只读 cache。

如果 cardinality 已知：

```text
First
unique lookup
```

则尽量：

```text
FindFirstBuildCache
```

而不是 FindAll 再 slice。

---

# 15. UIA First/Nth 当前没有 early-stop

`execute_expression` 里：

```text
First(query)
    -> execute_expression(query)
    -> results.into_iter().take(1)
```

也就是说：

```text
先把全部结果找完
再拿第一个
```

Nth 也是类似。

这对于大型 UI tree 明显浪费。

---

## 15.1 推荐把 cardinality hint 下传

例如：

```text
ExecutionLimit
 ├─ All
 ├─ First
 └─ AtMost(N)
```

编译后：

```text
First
 -> query(limit=1)

Nth(8)
 -> query(limit=8)
```

RawView traversal 一旦达到目标数量即可停止。

这类优化不改变任何外部语义，风险低，收益可能很明显。

---

# 16. UIA relation traversal 还有 O(N²) 风险点

`append_unique()` 当前：

```text
for incoming:
    destination.iter().any(runtime_id == candidate)
```

因此大量 relation roots / candidate 合并时，最坏去重复杂度接近 O(N²)。

目前有：

```text
max_relation_roots=256
max_traversal_nodes=10000
```

所以不是无限灾难，但仍是可优化热路径。

建议在一次 materialization 中维护：

```rust
HashSet<RuntimeId>
```

同时保留 Vec 输出顺序：

```text
seen: HashSet
ordered: Vec
```

这样插入近似 O(1)。

---

# 17. UIA selector wait 也可以事件化

当前 TargetNotFound：

```text
sleep(poll_interval)
re-materialize
```

简单可靠。

但对于频繁等待控件出现的自动化，仍然会不断查询 provider。

以后可以增加：

```text
WaitStrategy
 ├─ Poll
 └─ EventThenVerify
```

利用：

- StructureChanged
- WindowOpened
- PropertyChanged

事件只负责唤醒，不直接认为 selector 已满足：

```text
event
 -> rerun frozen plan
 -> verify
```

这样既保留现有查询语义，又降低空轮询。

需要注意：

- handler 必须和 apartment 生命周期匹配；
- 必须保证 unregister；
- provider 可能事件风暴；
- deadline/cancellation 必须独立存在。

所以我把它放在 bulk cache / PID pushdown 之后。

---

# 18. CDP vs UIA：谁更高性能？

没有一种后端在所有场景更快。

它们的物理成本模型完全不同。

| 维度 | CDP | UIA |
|---|---|---|
| IPC | WebSocket async | COM / provider cross-process |
| 浏览器 DOM | 极强 | 弱 |
| CSS | 原生强项 | 无 |
| Accessible semantics | Chromium AX 可很强，但当前 ArgusFlow 尚未真正接入 | 原生 UIA provider 语义 |
| 大批量抓取 | 页面内批处理非常强 | 应使用 BuildCache 才高效 |
| 并发 | 当前连接可 multiplex | 当前全局单 worker |
| native desktop | 不适用 | 核心强项 |
| 控件 pattern | DOM action / CDP | Invoke / Value / ExpandCollapse / Toggle / LegacyIAccessible 等 |
| target lifecycle | 当前事件被忽略，需补强 | HWND/PID/stale/recovery 已较强 |
| cold start | 独立 Chromium 较贵 | UIA runtime 本身较轻 |
| 崩溃隔离 | session/transport health 仍需加强 | generation recovery 较成熟 |
| selector wait | 当前 100ms 重跑页面脚本 | 当前 polling，未来可 event wake |
| 理论吞吐上限 | 高 | 受 provider/COM 限制，但仍可显著优化 |

### 我的判断

**浏览器内的数据抓取、DOM 操作，应优先把 CDP 做成真正的高性能主力。**

**桌面原生应用仍应以 UIA 为主，重点把“跨进程调用次数”和“全局串行”压下来。**

不要试图让 UIA 和 CDP 使用完全一样的内部实现。

应该统一的是：

```text
Action semantics
Planner capability
PreparedExecution
Error classification
Deadline / cancellation
Metrics
```

而不是统一底层查询机制。

---

# 19. 整体项目是否“低耦合高内聚”？

## 我的工程化评分

> 以下是静态代码审查的启发式评分，不是 benchmark。

| 项目 | 评分 |
|---|---:|
| crate 边界 | 8.5/10 |
| ActionBackend 抽象 | 9/10 |
| PreparedPlan / freeze model | 9/10 |
| Resource 生命周期抽象 | 8.5/10 |
| UIA robustness | 8.5/10 |
| CDP transport 架构 | 8/10 |
| CDP semantic executor 一致性 | 5/10 |
| UIA 当前吞吐结构 | 6/10 |
| Workflow node 扩展性 | 8.5/10（基线 6/10） |
| Resource type 扩展性 | 8.5/10（基线 6.5/10） |
| 总体低耦合/高内聚 | 约 8/10 |
| 面向未来快速插件扩展 | 约 8/10（基线约 7/10） |

为什么整体还能到较高评价？

因为真正困难的：

```text
query language
planner
backend
runtime resource ownership
COM thread boundary
CDP transport ownership
```

已经拆开。

基线的顶层 workflow feature registration 债务已经在 schema v7 中拆开。当前扩展性债务主要收敛为：图内 fan-out/join 语义、动态插件发现与前端插件渲染协议；它们不再阻塞普通内置节点和资源类型增长。

---

# 20. 已解决：WorkflowNodeKind 封闭枚举

`79e00a3` 基线是：

```rust
enum WorkflowNodeKind {
    Start,
    Log,
    Debug,
    Delay,
    Condition,
    Application,
    Browser,
    Ui,
    Command,
    End,
}
```

强类型非常舒服。

但每增加一种功能，通常意味着：

```text
core enum
runtime executor match
validator
resource/reference validation
frontend contracts
node catalog
node inspector
log label
serialization
Tauri contract
tests
```

都要动。

这不完全符合：

```text
Open for extension
Closed for modification
```

当前 schema v7 已改为：

```text
WorkflowNode
  └─ NodeEnvelope { type_id, version, payload }
          ↓ registry compile
PreparedNode
  ├─ validation
  ├─ value/resource/control ports
  ├─ AccessSet
  └─ execute
```

新增节点只需注册自己的 `NodeCompiler`；动态 payload 在 prepare 阶段解码一次，执行热路径不再读取 JSON 或进入中央节点 match。注册节点还可以拥有命名空间化校验问题码和节点执行错误，不需要借用内置节点语义。

---

# 21. Browser 提交是基线扩展成本样本

为了新增 Browser resource，当前提交需要修改：

```text
argusflow-core
argusflow-agent
argusflow-browser
argusflow-runtime
argusflow-windows
src-tauri
frontend contracts
editor components
tests
schema version
```

其中有一部分是功能第一次打通必然要改。

但如果未来每增加：

- Database node
- HTTP node
- Excel node
- OCR session
- SSH resource
- Mobile device
- WebSocket resource
- File watcher
- AI agent
- Loop/Map/Parallel
- plugin-defined node

都需要改一组中央 enum/switch，那么项目越大，扩展成本会近似线性上升。schema v7 的注册边界正是针对这个样本落地：业务模块仍要实现自己的功能，但不再要求所有已有节点、资源清理器和执行分发器同步修改。

---

# 22. 你喜欢 ECS：我认为可以借，但不要“照搬游戏 ECS”

你的直觉其实很适合这个项目。

ArgusFlow 现在已经有一些 ECS 的胚胎：

| 当前概念 | ECS 类比 |
|---|---|
| ResourceId | Entity ID |
| RunContext | World |
| ResourceTable | Component storage |
| ExecutionEvent | Event |
| ActionBackend | System |
| PreparedPlan | Scheduled system work |
| Capability | Query/System filter |

`79e00a3` 基线当时还差：

- 开放的 component/resource kind 注册；
- 开放的 system/node 注册；
- 基于 access set 的调度；
- definition 与 runtime component 的彻底分离。

这四项目前均已形成代码边界：`ResourceTypeId/ResourceTable`、`NodeTypeRegistry`、`AccessSet/ResourceScheduler` 和 `NodeEnvelope/PreparedNode`。图内并行仍有意暂缓，等待 join、错误传播和取消语义先定义完整。

---

# 23. 我推荐的不是“全 ECS”，而是三阶段 Hybrid ECS

## 阶段 1：Definition

持久化层可以变成：

```text
NodeEnvelope
 ├─ id
 ├─ type_id
 ├─ version
 ├─ position
 └─ payload
```

例如：

```json
{
  "id": "browser-1",
  "type_id": "argus.browser",
  "version": 1,
  "payload": {
    "executable_path": "...",
    "initial_url": "..."
  }
}
```

但注意：

> 不要让整个 Rust 运行时都变成 serde_json.Value。

Registry 在 load/compile 时立刻把 payload decode 成强类型。

---

# 24. 阶段 2：Compile / Prepare

注册表：

```text
NodeTypeRegistry
 ├─ descriptor
 ├─ schema
 ├─ validator
 ├─ compiler
 ├─ ports
 ├─ capabilities
 └─ optional frontend metadata
```

运行前：

```text
dynamic NodeEnvelope
      │ decode + validate
      ▼
typed node compiler
      │
      ▼
CompiledNode
 ├─ SystemId
 ├─ Prepared payload
 ├─ AccessSet
 ├─ Inputs
 └─ Outputs
```

关键思想：

> **动态扩展发生在 prepare 阶段；真正 execute 不再做动态 schema 查找。**

这和你现在的 `PreparedExecution` 哲学完全一致。

这是我认为 ArgusFlow 最适合借鉴 ECS 的地方。

---

# 25. 阶段 3：RunWorld + Systems

```text
RunWorld
 ├─ Run component
 ├─ Node state
 ├─ Value outputs
 ├─ Resource components
 ├─ Capability components
 ├─ Query plan cache
 ├─ Diagnostics
 └─ cancellation/deadline
```

Systems：

```text
ValidationSystem
ResourceAcquireSystem
UiPlanSystem
CdpSystem
UiaSystem
CommandSystem
EvidenceSystem
CleanupSystem
```

执行计划：

```text
CompiledGraph
      │
      ▼
Scheduler
      │
      ├─ read/write conflict check
      ├─ resource affinity
      ├─ side-effect ordering
      └─ concurrency budget
```

---

# 26. 一个更适合 ArgusFlow 的 ECS-inspired 核心接口

概念示意：

```rust
pub struct CompiledNode {
    pub system: SystemId,
    pub prepared: Arc<dyn PreparedNode>,
    pub access: AccessSet,
}

#[async_trait]
pub trait PreparedNode: Send + Sync {
    async fn execute(
        &self,
        ctx: NodeExecutionContext,
    ) -> Result<NodeOutcome, RuntimeError>;
}

pub struct AccessSet {
    pub reads: Vec<ResourceKey>,
    pub writes: Vec<ResourceKey>,
    pub exclusive: Vec<ResourceKey>,
}
```

Registry 负责：

```rust
pub trait NodeCompiler: Send + Sync {
    fn type_id(&self) -> NodeTypeId;

    fn compile(
        &self,
        envelope: &NodeEnvelope,
        context: &CompileContext,
    ) -> Result<CompiledNode, CompileError>;
}
```

这样：

```text
Browser node
Database node
HTTP node
UI node
Custom plugin node
```

不需要继续膨胀一个总 `match WorkflowNodeKind`。

---

# 27. ECS 调度时不要犯一个常见错误：UI 自动化不是纯数据计算

游戏 ECS 很容易：

```text
TransformSystem
PhysicsSystem
AnimationSystem
```

并行跑。

UI 自动化有外部副作用：

```text
click
focus
type
window activation
navigation
```

因此必须有 resource access semantics。

例如：

```text
CDP GetText(page A)     -> Read(page A)
CDP CollectLinks(page A)-> Read(page A)
CDP Click(page A)       -> Exclusive(page A)
UIA GetText(app A)      -> Read(app A)
UIA SetValue(app A)     -> Exclusive(app A)
Command                 -> capability-specific
```

Scheduler 只有在没有冲突时才能并行。

---

# 28. 已落地第一阶段：多 RunWorld 与资源访问调度

`79e00a3` 基线中的 `WorkflowEngine`：

```text
active_run: Option<Uuid>
```

一个 engine 同时只有一个 run。

并且执行路径是：

```text
node -> node -> node
```

对于桌面自动化产品，这种 deterministic 模型一开始非常合理。

但如果以后希望：

- 多工作流并发；
- 多浏览器并行；
- 不同应用并行；
- read-only browser fan-out；
- map over 100 URLs；

就需要把：

```text
Engine
```

拆成：

```text
RunManager
  ├─ RunWorld A
  ├─ RunWorld B
  └─ RunWorld C

Scheduler
ResourceArbiter
Backend executors
```

**同一个 app/page 的 mutation 仍顺序执行，不同资源可以并行。**

这比“整个 engine 只能跑一个 workflow”更接近高吞吐 runtime。

当前实现已将 `active_run: Option<Uuid>` 替换为活动 RunWorld 集合，并在共享 `ResourceScheduler` 中按稳定资源键获取 read/exclusive 访问权。不同资源的多个工作流可以并存；相同 app/page 的 mutation 与资源 cleanup 仍保持互斥。图内路径继续顺序执行，避免在 join/cancellation 尚未定义时引入不确定副作用。

---

# 29. 已解决：开放 CapabilitySet

`79e00a3` 基线中的 `AppCapabilities` 是 bool fields：

```text
windows_uia
browser_cdp
visual
command_adapter
```

以后每加一种能力就可能继续加字段。

可以逐步变成：

```text
CapabilitySet
```

例如 interned capability id / compact bitset：

```text
ui.windows.uia
browser.cdp
vision.screen
command.adapter
browser.network
browser.download
```

Planner system 声明：

```text
requires = { browser.cdp }
```

这就是很 ECS 的 data-oriented filtering。

当前已使用开放 `CapabilityId + CapabilitySet`。内置能力用静态 ID 避免分配，外部提供器可以声明命名空间化能力；新增能力不再增加会话结构字段。

---

# 30. 已解决：BackendPolicy 开放集合

基线偏好大致：

```text
Auto
WindowsUia
BrowserCdp
```

但 Router 本身已经知道：

- VisualCache
- OCR
- GuiGrounding
- SendInput

以后如果这些都真正可用，继续扩 enum 会变得笨重。

建议未来让用户约束更像：

```text
BackendPolicy {
    allow: [...]
    deny: [...]
    prefer: [...]
}
```

Planner 自己仍按：

```text
support
availability
context
cost
priority
```

排序。

当前目标契约已采用 `BackendPolicy { allow, deny, prefer }`。`deny/allow` 只负责候选过滤，`prefer` 位于完整 AQL 分支路径、语义支持、可用性、上下文和成本之后，不会把后一个 `any(...)` 分支提前到前一个显式分支之前。

---

# 31. 已解决：Resource 类型与 cleanup 开放化

`79e00a3` 基线是：

```text
ResourceEntry
 ├─ Application
 └─ Browser
```

cleanup 也是 match。

可以变成：

```text
ResourceInstance {
    id,
    kind,
    state: Arc<dyn RuntimeResource>
}
```

配套：

```text
ResourceProviderRegistry
```

但这里要注意性能：

> Registry/type-erasure 只出现在资源边界，不要在 CDP/UIA 热循环中每个 element 都做 Any/HashMap<TypeId>。

ECS 的灵活性应该用在：

- resource composition；
- node registration；
- scheduler；
- capability filtering。

不是用来把每次 selector predicate 都变成动态反射。

当前 `ResourceTable` 只在实例边界保存 `ResourceTypeId + Arc<dyn Any> + ResourceCleanup + ResourceAccessKey`；节点恢复具体类型后继续走强类型 API。清理策略随资源实例冻结，新增 Database/SSH/Mobile 等资源无需修改中央 enum 或 cleanup match。

---

# 32. 我最推荐的最终架构

```text
                    ┌──────────────────┐
                    │ Workflow JSON    │
                    │ NodeEnvelope     │
                    └────────┬─────────┘
                             │
                     registry decode
                             │
                             ▼
                    ┌──────────────────┐
                    │ Compiler         │
                    │ Validator        │
                    │ Query compiler   │
                    └────────┬─────────┘
                             │
                             ▼
                ┌──────────────────────────┐
                │ Frozen ExecutionPlan     │
                │ - typed prepared nodes   │
                │ - resource access sets   │
                │ - backend candidates     │
                │ - deadlines              │
                └───────────┬──────────────┘
                            │
                            ▼
                ┌──────────────────────────┐
                │ RunWorld                 │
                │ compact runtime state    │
                └───────────┬──────────────┘
                            │
                            ▼
                ┌──────────────────────────┐
                │ Scheduler / Systems      │
                └──────┬───────────┬───────┘
                       │           │
             ┌─────────▼───┐   ┌───▼──────────┐
             │ UiaWorkerPool│   │ CdpRuntime   │
             │ app affinity │   │ event actor  │
             └──────────────┘   └──────────────┘
```

核心理念：

```text
Open definition
Strong compile
Frozen execution
Data-oriented runtime
Resource-aware concurrency
```

我认为这比完整套用 Bevy/Specs 风格 ECS 更适合 ArgusFlow。

---

# 32.1 本轮已落地的扩展性改造

本轮已经把上述设计中的主要“功能扩展阻塞项”落到代码：

```text
Workflow schema v7
  └─ NodeEnvelope { type_id, version, payload }
          │
          ▼
NodeTypeRegistry
  └─ NodeCompiler::compile
          │
          ▼
PreparedNode
  ├─ typed payload
  ├─ validation
  ├─ value/resource ports
  ├─ control ports
  ├─ AccessSet
  └─ execute
```

具体变化：

1. 删除后端 `WorkflowNodeKind` 封闭枚举；新增节点通过 `NodeTypeId + NodeCompiler` 注册，不再修改 Runtime 中央 match。
2. 动态 `payload` 只在启动前解码；执行路径只持有强类型 `PreparedNode`。
3. 控制流分支改为开放 `ControlPortId`，注册节点可以声明自己的端口集合。
4. 值端口改为开放 `ValueTypeId`，资源端口使用开放 `ResourceTypeId`；引用和支配校验统一消费端口声明。
5. `ResourceTable` 只在资源边界做一次类型擦除，资源实例同时冻结自己的清理策略和调度键；新增资源不再修改 cleanup match。
6. `CapabilitySet`、`BackendPolicy` 和 `WorkflowPermissions.allow` 改为开放集合，新增能力不再增加 bool 字段或偏好枚举分支。
7. 前端默认值和 NodeEnvelope 编码合并到同一 definition codec 注册表。
8. `WorkflowEngine` 已允许多个 RunWorld 并存，并通过 `AccessSet` 的 read/exclusive 语义仲裁跨运行资源冲突。

当前仍然有意保留的边界：

- 单个条件 DAG 的命中路径仍按节点顺序执行，保证桌面自动化副作用确定性；
- CDP/UIA selector 热循环继续使用专用强类型计划，不进入动态 registry；
- 图内 fan-out 并行需要先定义 join/error/cancellation 语义，不能仅靠 `spawn` 扩散节点。

---

# 33. CDP 具体重构路线

## P0：先保证 Planner 说真话

1. [已完成] 非 CSS semantic matcher 在真正 AX/DOM executor 完成前如实降级；
2. [已完成] `CdpMatcherPlan.source` 进入 executor 并决定实际候选来源；
3. [已完成] transport/detach/crash 按结构化 failure/lifecycle 分类映射为可 fallback 的 unavailable；
4. [待完成] 添加真实性能 metrics。

---

## P1：实现真正 native source executor

### CSS

继续走快路径。

### DOM

尽可能把稳定 selector 条件合成原生 DOM selector，或用 DOM domain 做 candidate narrowing。

### AX

使用 Chromium Accessibility domain：

```text
Accessibility.queryAXTree
```

按真实 accessible name / role 找候选。

只有 residual 才拉取额外数据。

---

## P1：CDP event actor

增加：

```text
Target events
Runtime context events
Page lifecycle
optional Network
```

CdpPageSession 有真正的 health state。

---

## P1：in-flight & cancellation

```text
bounded mailbox
+ semaphore
+ deadline
+ cancel id
```

---

## P1：把 page interpreter 从“每请求传 13KB 脚本”改为 helper

可考虑每 execution context 注入：

```javascript
globalThis.__argusflow = { execute(plan, action) { ... } }
```

之后调用只传：

```text
small plan JSON
small action JSON
```

navigation/context destroyed 后重新注入。

这会降低：

- message serialization；
- JS parse；
- retry payload；
- CPU churn。

如果未来用 CDP native executor更多，helper 甚至只负责 residual/action projection。

---

## P2：Browser pool

可选：

```text
process pool
browser context pool
page pool
```

默认仍保持 dedicated isolation。

---

# 34. UIA 具体重构路线

## P0

1. [已完成] discovery condition 下推 `ProcessId`；
2. [已完成] 下推可安全表达的 native equality；
3. [已完成] 使用 `FindAllBuildCache / FindFirstBuildCache`；
4. [已完成] 通过 BuildCache 减少 `Current*` IPC；
5. [已完成] First/Nth 按结果上限 early-stop；
6. [已完成] runtime-id dedupe 改为完整 ID 的 `HashSet`。

---

## P1

### UiaWorkerPool

```text
AppSession/Process affinity
+ bounded queue
+ per-shard generation recovery
```

同 app mutation 串行，不同 app 并行。

---

## P1

关系遍历使用 BuildCache TreeWalker API，把：

```text
GetChild
CurrentProperty
CurrentProperty
```

变为尽量少的 IPC。

---

## P2

事件唤醒 selector wait。

---

# 35. 建议建立统一的 Backend Execution Budget

现在：

- UIA 有 25s execution timeout；
- CDP action 内部自己固定 5s；
- Browser acquire 有 launch timeout；
- query compiler 有 alternative budget。

建议统一成：

```text
ExecutionBudget
 ├─ absolute deadline
 ├─ queue budget
 ├─ max candidates
 ├─ max visited nodes
 ├─ max relation roots
 ├─ max IPC roundtrips
 └─ cancellation token
```

PreparedExecution 接收的是共享 budget：

```text
Workflow deadline
      │
      ▼
Node deadline
      │
      ▼
Backend deadline
```

而不是每个 backend 自己发明一个 timeout。

这会让：

```text
UIA
CDP
OCR
Vision
Command
```

行为更一致。

---

# 36. 建议建立统一 Metrics，性能优化才不会靠感觉

如果目标是“高性能、高效率”，我非常建议把下面指标变成正式运行指标。

## Planner

```text
planner_prepare_us
query_compile_us
candidate_count
selected_backend
fallback_count
```

## CDP

```text
cdp_queue_us
cdp_inflight
cdp_roundtrips
cdp_bytes_out
cdp_bytes_in
cdp_dom_visited
cdp_eval_us
cdp_event_lag
```

## UIA

```text
uia_queue_us
uia_provider_calls
uia_current_property_calls
uia_cached_property_reads
uia_traversal_nodes
uia_relation_roots
uia_materialize_us
uia_action_us
uia_worker_generation
uia_recovery_count
```

## Runtime

```text
node_wait_us
node_execute_us
resource_lock_wait_us
run_p50/p95/p99
```

只有有这些，之后的 ECS/worker pool/BuildCache 优化才能证明有效。

---

# 37. 建议做的 benchmark matrix

本报告没有在本地 Windows 环境实际跑 benchmark，因此下面是强烈建议补的验证矩阵。

## CDP

DOM sizes：

```text
1k / 10k / 50k
```

Query：

```text
css exact
semantic role
role+name
descendant
not
first
nth
collectLinks 10/100/1000
```

Concurrency：

```text
1 / 8 / 32 / 128
```

测：

```text
P50
P95
P99
CPU
RSS
pending count
round trips
```

---

## UIA

应用：

```text
Win32
WPF
WinUI
Notepad++
custom provider
```

tree：

```text
small
1k
5k
10k+
```

对比：

```text
ControlType only
+ ProcessId condition
+ native props
FindAll
FindAllBuildCache
FindFirstBuildCache
single worker
sharded worker
```

重点测：

```text
COM call count
queue time
provider time
full selector latency
```

---

# 38. 如果只允许我挑 10 个改动，我会按这个顺序

1. **[已完成] 修复 CDP compiler/executor 语义不一致。**
2. **[已完成] CDP transport/session failure 正确映射到 fallback。**
3. **[已完成] UIA discovery condition 加 ProcessId。**
4. **[已完成] UIA 改用 FindAllBuildCache / FindFirstBuildCache。**
5. **[已完成] UIA First/Nth early stop。**
6. **CDP 加真正 in-flight cap + cancellation。**
7. **CDP 加 event dispatch 与 session health。**
8. **UIA 改 bounded mailbox，并做 AppSession-affinity worker pool。**
9. **[已完成] Workflow Node/Resource 注册表化，删除中央 enum/switch。**
10. **[已完成第一阶段] 引入 ECS-inspired RunWorld + AccessSet scheduler，并保留 PreparedPlan。**

---

# 39. 哪些地方我反而不建议重构

## 39.1 不要扔掉 PreparedExecution

这是目前架构里最有价值的东西之一。

它天然实现：

```text
dynamic planning
      │
      ▼
frozen plan
      │
      ▼
cheap execution
```

这其实和高性能 ECS 的“schedule/compile once, execute many”思想非常接近。

---

## 39.2 不要把所有东西都改成 Any/JSON

如果把：

```text
Action
Query
Resource
Node
```

全部改成：

```text
HashMap<String, Value>
```

会立刻失去当前项目最大的优势：

- Rust strong typing；
- compiler proof；
- exhaustiveness；
- predictable executor。

正确做法是：

```text
开放注册边界
        │
        ▼
decode/compile
        │
        ▼
强类型 frozen runtime
```

---

## 39.3 不要为了“并发”并发所有 UI 动作

UI automation 是有 side effect 的。

你真正要的是：

```text
resource-aware concurrency
```

不是：

```text
spawn every node
```

---

# 40. 架构最终评价

## CDP

### 架构

**好，方向对。**

Transport/session/resource ownership 明显是经过思考的。

### 当前性能

- CSS / bulk extraction：不错；
- async transport：不错；
- semantic AQL：目前还有明显浪费；
- browser cold start：隔离换性能，可做可选 pooling。

### 最大不足

**真正的 AX/DOM native semantic executor 尚未完成。**

当前 compiler/Explain 已如实标记实际 DOM 页面解释器路径，transport/session failure 也已结构化分类；下一步性能收益来自实现真实 `Accessibility.queryAXTree` / DOM narrowing，而不是继续修正声明口径。

---

## UIA

### 架构

**稳健性非常不错。**

专用 COM apartment、generation recovery、预算、stale retry 都是高质量工程决策。

### 当前性能

**保守，吞吐还有较大优化空间。**

核心瓶颈：

- 单全局 worker；
- unbounded queue；
- 部分关系遍历仍经 TreeWalker；
- 尚未按 AppSession/process affinity 分片；
- queue time、provider time 与 COM call count 指标不足。

ProcessId/native equality pushdown、`FindFirst/FindAllBuildCache`、First/Nth early-stop 和 `HashSet` runtime-id 去重已经完成，不再列为现状瓶颈。

---

## 整体框架

### 低耦合高内聚

已经明显高于普通工作流/RPA 项目。

尤其：

```text
core
query
agent
runtime
browser/windows
```

职责边界比较清楚。

### 抽象程度

后端层够抽象；

Workflow node/resource/capability registration 已经打开，并在 prepare 阶段冻结为强类型执行对象。

### 快速扩展新功能能力

本轮改造后：

```text
新增一个 ActionBackend     -> 通过现有后端注册边界接入
新增一种完整 Node/Resource -> 注册 NodeCompiler、端口和清理策略，不改中央枚举/switch
```

下一步真正值得投资的是：

> **在已打开的 Node/Resource/Capability 注册体系上补图内 fan-out/join 语义与可观测调度指标。**

---

# 41. 最后一句建议

如果 ArgusFlow 最终想成为一个很强的、可持续扩展的自动化运行时，我会把目标定成：

> **“ECS 的扩展性 + 编译型 Planner 的确定性 + CDP 的异步高吞吐 + UIA 的平台语义与容错。”**

而不是纯 RPA：

```text
节点 -> switch -> 调 API
```

也不是纯游戏 ECS：

```text
所有数据都 component 化，所有行为都 system scan
```

最适合它的模型是：

```text
Registry-driven Definition
        ↓
Strongly Typed Compile
        ↓
Frozen ExecutionPlan
        ↓
ECS-inspired RunWorld
        ↓
Resource-aware Scheduler
        ↓
Specialized CDP/UIA Systems
```

经过本轮 schema v7 与调度边界改造，当前代码已有约 75%-80% 的“正确地基”；剩余工作主要集中在图内并行语义和平台后端吞吐，而不是新增普通节点必须修改中央契约。

真正需要补的是：

- CDP 真正 AX/DOM native plan 的实现；
- UIA IPC/worker throughput；
- 图内并行的 join/error/cancellation 语义；
- runtime resource-aware concurrency 的压力验证与指标。

这几件做好以后，架构会从“不错的自动化框架”明显跨到“可扩展自动化引擎”。

---

# 42. 审查范围与限制

本报告基于 GitHub `main` 最新提交 `79e00a392c86af84d1c1c621174eec5c9f514db7` 的代码静态审查。

重点实际阅读了：

```text
crates/argusflow-browser/src/backend.rs
crates/argusflow-browser/src/runtime.rs
crates/argusflow-browser/src/cdp/protocol.rs
crates/argusflow-browser/src/cdp/session.rs
crates/argusflow-browser/src/cdp/compiler.rs
crates/argusflow-browser/src/cdp/plan.rs
crates/argusflow-browser/src/cdp/executor.rs
crates/argusflow-browser/src/cdp/page_script.rs
crates/argusflow-browser/src/cdp/explain.rs

crates/argusflow-windows/src/uia/backend.rs
crates/argusflow-windows/src/uia/runtime.rs
crates/argusflow-windows/src/uia/runtime_worker.rs
crates/argusflow-windows/src/uia/executor.rs
crates/argusflow-windows/src/uia/process_search.rs
crates/argusflow-windows/src/uia/condition.rs
crates/argusflow-windows/src/uia/element_search.rs
crates/argusflow-windows/src/uia/cache.rs
crates/argusflow-windows/src/uia/target_selection.rs

crates/argusflow-agent/src/backend.rs
crates/argusflow-agent/src/router.rs
crates/argusflow-agent/src/plan.rs

crates/argusflow-core/src/resource.rs
crates/argusflow-core/src/workflow.rs

crates/argusflow-runtime/src/engine.rs
crates/argusflow-runtime/src/node_registry.rs
crates/argusflow-runtime/src/scheduler.rs
```

同时参考：

- Chrome DevTools Protocol Target domain；
- Chrome DevTools Protocol Accessibility domain；
- Microsoft UI Automation caching 文档；
- Microsoft UI Automation element search 文档；
- `FindAllBuildCache / FindFirstBuildCache` 官方 API 文档。

审查时没有看到该最新 commit 关联的 GitHub commit status / workflow run 结果。

因此：

- 架构判断：可信；
- 复杂度与 IPC 热点判断：可信；
- “哪个方案实际快多少”：需要 benchmark 证明；
- 本报告没有声称已在真实 Windows/Chromium 环境替你跑完性能基准。

---

# 43. 参考链接

- Repository: https://github.com/SLOE-debug/argusflow
- Reviewed commit: https://github.com/SLOE-debug/argusflow/commit/79e00a392c86af84d1c1c621174eec5c9f514db7
- CDP Target: https://chromedevtools.github.io/devtools-protocol/tot/Target/
- CDP Accessibility: https://chromedevtools.github.io/devtools-protocol/tot/Accessibility/
- Microsoft UIA caching: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-cachingforclients
- Microsoft UIA obtaining elements: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-obtainingelements
- FindAllBuildCache: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-findallbuildcache
- FindFirstBuildCache: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-findfirstbuildcache
