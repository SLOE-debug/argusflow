# ArgusFlow Selector 韧性与失败取证抽象层方案

> 基于仓库 `SLOE-debug/argusflow` 当前 `main`：`e86380d2cbd2b762ab49d2452673f7f9d6ca0f6c`（2026-08-26）
>
> 目标：解决真实 UIA / CDP / OCR 自动化中「定位器随 provider、语言、时序、UI fragment 变化而失效」以及「selector 失败后缺少现场证据」两个问题。
>
> 本方案遵守现有仓库边界：保留 AQL、PreparedPlan、ActionRouter、backend compiler / executor 分层；不再造第二套 selector DSL，不把 UIA / CDP / OCR 细节泄漏进 `argusflow-core`。

---

## 1. 结论先行

目前 Notepad++ E2E 已经从 `Item 43001`、主 HWND 子树、单一 Pattern 等不可靠假设，升级到了：

- PID 作用域的 UIA fragment 搜索；
- 中文 Accessible Name；
- `InvokePattern / ExpandCollapsePattern / LegacyIAccessiblePattern` 能力选择；
- `ValuePattern` 写值；
- `TargetNotFound / AmbiguousTarget` 明确分离。

这个方向是对的，但还可以再向前收敛成两个正式能力：

1. **Selector Resilience（定位韧性）**
   - 不追求一个“永远不会坏”的 selector；
   - 使用确定性的稳定身份信号 + 明确的 AQL `any(...)` fallback；
   - 对短暂未出现的 UI 使用同一 PreparedPlan 的有界 wait / poll；
   - 让 action capability 参与“目标是否可执行”的判定，但不把能力缺失误报成 TargetNotFound；
   - 运行时不做模糊打分，不偷偷 `first()` / `nth()`。

2. **Failure Evidence（失败取证）**
   - 每个 PreparedCandidate 可选地携带一个 PreparedDiagnostics；
   - selector 分支失败时，在 fallback 发生前捕获现场；
   - UIA 输出 process-scoped tree / candidate explain / pattern capability / screenshot；
   - CDP 输出 DOMSnapshot / AX Tree / screenshot；
   - OCR 输出真正参与推理的原始帧 / ROI / boxes / confidence / overlay；
   - 全部落成统一 Evidence Bundle，有结构化 manifest，而不是只打印 stdout。

最重要的一点是：

> **ArgusFlow 已经有 selector fallback 的核心语义，不需要再设计一套 fallback 系统。**
>
> AQL `any(...)` 已经按声明顺序形成 `branch_path`；backend compiler 会为每条路径生成独立候选；`PreparedPlan` 对 `TargetNotFound` 会进入更晚分支。应当强化这套机制，而不是在 UIA executor 内堆特殊 retry。

---

# 2. 当前实现里已经存在的正确基础

## 2.1 AQL 已经有显式 fallback

`argusflow-core/src/query.rs` 中的 `QueryExpr::Any` 本身就是“按声明顺序组合多个可替代查询，顺序代表回退优先级”。

`argusflow-windows/src/uia/compiler.rs` 的 `compile_any()` 会：

- 展开每个可执行分支；
- 保留原始分支顺序；
- 将分支索引写入 `BranchPath`；
- 一条 PreparedCandidate 只绑定一条完整 fallback 路径。

`argusflow-agent/src/plan.rs` 的 `PreparedPlan::execute()` 已经具有关键语义：

- `BackendUnavailable`：允许同一路径继续换 backend；
- `TargetNotFound`：当前 selector 分支耗尽，进入更晚 `branch_path`；
- 其他错误：不偷偷 fallback。

因此后续 selector 韧性应建立在：

```text
AQL any(...)
    ↓
Backend compiler
    ↓
PreparedCandidate(branch_path)
    ↓
PreparedPlan deterministic fallback
```

而不是：

```text
UIA executor 内：
if selector A fail -> 拼 selector B -> 再试 -> 再猜
```

后者会破坏 Explain、可审计性和 backend 一致性。

---

## 2.2 当前 UIA executor 已经解决 HWND fragment 边界

`argusflow-windows/src/uia/executor.rs` 初始搜索范围已经是：

```rust
SearchScope::Process { process_id }
```

而不是只从冻结主 HWND 往下找。

这与菜单 popup、Find dialog 可能处于独立 UIA fragment 的事实一致。

后续 Failure Evidence 必须沿用**真实执行范围**：

> UIA 失败快照默认也必须以 Desktop root + ProcessId 为边界，而不能退回到 `ElementFromHandle(main_hwnd)` 的单一 ControlView dump。

否则会出现最糟糕的一类诊断：executor 明明搜索了 popup fragment，但 dump 里根本没有 popup，于是证据与实际执行路径不一致。

---

## 2.3 当前动作能力选择是正确方向

`argusflow-windows/src/uia/action.rs` 已经不是“看到 MenuItem 就 Invoke”：

```text
InvokePattern available?
  ↓ no
ExpandCollapsePattern available?
  ↓ no
LegacyIAccessiblePattern available?
```

这比绑定应用命令 ID、坐标、控件类型假设稳定得多。

下一步应当把这套“动作能力”从**最终 action 阶段**向前扩一小步：

- 查询先找出 selector 的语义候选；
- 再根据 Prepared Action 过滤“不可能执行该动作”的候选；
- 然后做 0 / 1 / N 唯一性解析。

但错误类型必须保持严格区分，见第 6 节。

---

## 2.4 已有 UIA dump 是很好的种子，但还只是 test helper

当前：

```text
crates/argusflow-windows/tests/support/uia_dump.rs
```

已经会输出：

- ControlType
- Name
- AutomationId
- ClassName
- enabled
- offscreen

并且 `NotepadPlusPlus::drop()` 在 panic 时会 dump。

这证明“失败取证”不是空抽象，而是仓库已经真实需要、只是还没有提升成正式运行时能力。

其主要不足：

1. 只存在于 tests；
2. 只支持 UIA；
3. 只在 panic 时触发；
4. 以 HWND ControlView 为根，可能漏独立 fragment；
5. 只给 raw tree，没有告诉你“这个 selector 为什么失败”；
6. 输出到 stderr，没有稳定 artifact schema；
7. fallback 成功时，前一个 selector 的脆化会被隐藏。

---

# 3. “查找(F)...”有没有更硬的定位方式？

答案是：**有，但不建议寻找一个万能替代字段，而应建立稳定身份阶梯。**

## 3.1 不建议回到 `Item 43001`

`AutomationId` 在 UIA 中很有价值，但不是“任何 provider、任何 client 都必须给”。

微软对 MenuItem 的规范明确允许动态、不可预测的菜单项把 AutomationId 留空；UIA 自动化测试文档也明确指出 AutomationId 并非强制支持，并且不保证跨应用版本永远不变。

这与你已经观察到的行为完全吻合：

```text
.NET client + ClientSideProviders
    → Item 43001

native CUIAutomation8
    → 同一元素 AutomationId 为空
```

所以：

```text
uia.automation_id = "Item 43001"
```

可以是某些应用 / provider 上的一级稳定 selector，但不能成为 ArgusFlow 对传统 Win32 菜单的通用答案。

---

## 3.2 对“查找”菜单项，优先实测 `AcceleratorKey`

UIA 有一个比 Name 中的 `\tCtrl+F` 更语义化的属性：

```text
AcceleratorKey
```

它表达的是“触发该动作的快捷键组合”，例如：

```text
Ctrl+F
```

对于“查找”这种典型命令，这比：

```text
name starts_with "查找(F)..."
```

更接近稳定身份：

- 不依赖 `\tCtrl+F` 是否被拼进 Name；
- 不依赖 `(F)` mnemonic 的格式；
- 对本地化语言更友好；
- 是标准 UIA property，而不是应用私有命令号。

**但一定要先从 native CUIAutomation8 实测 Notepad++ 是否真实公开该属性。**

不要因为规范里有这个属性，就假设 Win32 / MSAA bridge 一定填了它。

建议 AQL 增加：

```rust
pub enum UiaAttribute {
    AutomationId,
    ClassName,
    AcceleratorKey,
    AccessKey,
    FrameworkId,
}
```

其中第一优先级是 `AcceleratorKey`；`AccessKey` / `FrameworkId` 主要用于诊断和组合约束，不建议为了“属性更多”一次性暴露所有 UIA PropertyId。

理想写法：

```text
menu_item(
    uia.accelerator_key = "Ctrl+F"
)
```

如果 provider 不提供，再明确 fallback：

```text
any(
    menu_item(uia.accelerator_key = "Ctrl+F"),
    menu_item(name = "查找(F)...\tCtrl+F"),
    menu_item(name starts_with "查找(F)...")
)
```

> 上面第二、三分支仍然是本地化 fallback，但它们不再承担“唯一事实来源”的职责。

---

## 3.3 `AccessKey` 与 `AcceleratorKey` 要区分

对于 UIA：

```text
AccessKey
```

通常更接近菜单 mnemonic，例如 Alt + 某字母；

```text
AcceleratorKey
```

更接近命令快捷键，例如 Ctrl+F。

针对 Find：

```text
Ctrl+F
```

通常比 `(F)` 这个 UI 文本修饰更有价值。

因此不要把“把 `(F)` 从 Name 里 strip 掉”作为核心抽象。

---

## 3.4 不建议新增一个自动“清洗 Name”的魔法规则

可以很自然地想到：

```text
"查找(F)...\tCtrl+F"
        ↓ normalize
"查找"
```

但一个全局 `normalize_accessible_name()` 很危险：

- `(F)` 不一定永远是 mnemonic；
- `...` 可能是产品文案真实组成部分；
- `\t` 后内容可能来自 provider 的不同格式；
- 不同平台对 Accessible Name 的组成规则不同；
- CDP AX Name 与 UIA Name 不应共享 Win32 专用清洗逻辑。

所以 AQL 的 portable `name` 应继续表示真实 Accessible Name 语义。

如果未来确实要提供 normalization，应当：

- 放在 backend-specific projection；
- 明确 Explain 它做了什么；
- 不覆盖原始 Name；
- 不成为默认等值比较行为。

当前阶段不建议做。

---

# 4. 推荐的 Selector Identity Ladder

把 selector 的“硬度”看成多层证据，而不是一个字段。

## 4.1 第一层：稳定 backend-native ID

UIA：

```text
uia.automation_id
uia.accelerator_key
```

CDP：

```text
dom.test_id
稳定 DOM id / 明确 CSS escape hatch
```

特点：

- 值精确；
- 唯一性通常高；
- 不需要模糊匹配；
- 但必须真实存在，不能假设 provider 会提供。

---

## 4.2 第二层：语义角色 + 稳定 Accessible Name

例如：

```text
button(name = "取消")
```

或：

```text
dialog(name = "查找") >> textbox(name = "查找目标(F) :")
```

比只查字符串更好，因为 role 和关系共同缩小空间。

---

## 4.3 第三层：关系约束

例如：

```text
menu(...) >> menu_item(...)
```

或：

```text
dialog(name = "查找") >> textbox(...)
```

关系约束通常比全桌面 prefix 查询稳定，因为它表达“属于什么上下文”。

但不要用：

```text
nth(3)
```

当成“硬化”。顺序是 UI 重构时最容易变化的属性之一。

---

## 4.4 第四层：明确的文本 fallback

例如：

```text
name starts_with "查找(F)..."
```

它可以保留，但应降级成 fallback，而不是主 selector。

---

## 4.5 不要在 runtime 做模糊打分选择

不建议把 selector runtime 改成：

```text
AutomationId +50
Name 相似 +30
位置接近 +10
然后最高分就是目标
```

这会破坏 ArgusFlow 目前非常重要的安全属性：

```text
0 → TargetNotFound
1 → execute
>1 → AmbiguousTarget
```

“selector 稳定性评分”可以存在，但应该只用于：

- Inspector；
- lint；
- selector authoring；
- 自动建议候选。

不能成为隐式运行时决策。

---

# 5. 再进一步：Command 不一定应该伪装成 UI Element

“打开查找对话框”本质上是一个应用命令：

```text
Find command
```

UI 菜单只是这个命令的一种入口。

因此长期可以考虑把：

```text
Ctrl+F
```

建模为独立动作，而不是为了执行命令一定先定位 MenuItem。

例如未来：

```rust
UiOperation::Shortcut {
    chord: KeyChord,
}
```

或更高层：

```rust
UiOperation::Command {
    command: ApplicationCommand,
}
```

但这个动作**不能作为 selector 失败后的隐式 fallback**。

原因：

- UIA Click 和 SendInput Shortcut 的副作用模型不同；
- 当前窗口焦点会影响 shortcut；
- Explain 必须明确用户最终执行的是哪种机制。

因此推荐：

- UIA E2E 继续测试 MenuItem provider 能力；
- 产品工作流若目标只是“打开 Find”，可以允许用户显式选择 Shortcut 动作。

---

# 6. Action-aware Target Suitability

现在 UIA executor 的顺序大致是：

```text
query
  ↓
resolve_unique
  ↓
action pattern dispatch
```

这会产生一种可能的假歧义：

```text
selector 匹配 2 个元素
其中 1 个根本不支持当前动作
→ 仍然 AmbiguousTarget
```

可以改成：

```text
query semantic candidates
  ↓
action suitability filter
  ↓
resolve_unique
  ↓
execute exact pattern strategy
```

例如 Click 的可执行能力集合：

```text
InvokePattern
OR ExpandCollapsePattern
OR LegacyIAccessiblePattern
```

SetValue：

```text
ValuePattern && !ReadOnly
```

但错误必须细分：

```rust
pub enum TargetResolutionFailure {
    NotFound,
    Ambiguous { matches: usize },
    ActionUnsupported {
        semantic_matches: usize,
        required: ActionCapability,
    },
}
```

不能把：

```text
selector 找到了元素，但它没有 ValuePattern
```

变成：

```text
TargetNotFound
```

否则 PreparedPlan 会错误地进入更晚 selector branch，掩盖真正的问题。

### 结论

Action-aware filtering 是值得做的，但它是：

> **target suitability refinement**

不是：

> **更宽松的 selector fallback**。

---

# 7. 时序：用 bounded wait 替代固定 sleep

你已经确认 Toolbar 延迟不是持续失败主因，但当前 E2E 仍有：

```text
sleep 300ms
sleep 500ms
```

固定 sleep 的两个问题：

1. 快机器浪费时间；
2. 慢机器仍然偶发失败。

建议引入：

```rust
pub struct TargetWaitPolicy {
    pub timeout: Duration,
    pub poll_interval: Duration,
}
```

执行逻辑：

```text
Prepared selector branch
    ↓
execute same compiled plan
    ├─ found → continue
    └─ not found & before deadline
          ↓
        short sleep / event wakeup
          ↓
        execute same compiled plan again
    ↓ deadline
TargetNotFound
    ↓
capture evidence
    ↓
move to later branch_path
```

关键约束：

> **wait 是同一 PreparedPlan 的重复 materialize，不重新 parse、不重新 plan。**

这样不违反仓库现有“prepare 后冻结执行语义”的边界。

### UIA event 还是 polling？

长期可以利用 UIA structure/window events 唤醒；但真实 Win32 provider 的事件质量并不统一。

P0 推荐：

```text
50–100ms polling + 总 deadline
```

简单、可测、与现有 UiaExecutionBudget 思路一致。

### 何时取证？

应当在 wait budget 最终耗尽后取证，而不是第一次 miss 就 dump 一棵尚未稳定的树。

---

# 8. Failure Evidence：统一失败取证抽象

## 8.1 为什么不叫 `TreeDumpProvider`

用户真正需要的不是“树”，而是：

```text
失败现场证据
```

不同 backend 的事实来源完全不同：

- UIA：Accessibility tree + Pattern + PID/HWND fragment；
- CDP：AX tree + DOM tree + frame/session；
- OCR：输入帧 + ROI + recognition result；
- Grounding：输入图 + proposals / coordinates；
- SendInput：没有 selector tree，更多是 foreground/focus/input context。

因此统一抽象应该是：

```text
Failure Evidence
```

而不是：

```text
Tree Dump
```

---

## 8.2 不建议直接把诊断方法塞进 `ActionBackend`

当前 `ActionBackend` 的职责很干净：

```rust
fn kind(&self) -> BackendKind;
fn prepare(...) -> Result<Vec<PreparedCandidate>, PlanRejection>;
```

失败取证需要的是**Prepared 状态**：

- 哪个 branch_path；
- 哪个 frozen query plan；
- 哪个 HWND / PID；
- 哪个 CDP session / frame；
- 哪张 OCR input frame；
- 哪个动作能力。

因此更合适的边界是：

```rust
PreparedCandidate
├── PreparedExecution
└── PreparedDiagnostics (optional)
```

这比：

```rust
ActionBackend::dump_tree()
```

更符合当前架构。

---

# 9. 推荐强类型契约

建议先放在：

```text
crates/argusflow-agent/src/evidence.rs
```

当前 CDP 与 Vision executor 仍是占位，因此此时**不建议为了它单独拆一个新 crate**。

等至少 UIA + CDP 都有真实 collector，再评估是否抽成 `argusflow-observe`。

## 9.1 Trigger

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceTrigger {
    TargetNotFound,
    AmbiguousTarget,
    ActionUnsupported,
    BackendUnavailable,
    Timeout,
}
```

不建议所有 AutomationError 都默认截图。

例如 invalid AQL 是 compile-time 问题，不需要 UI snapshot。

---

## 9.2 Capture policy

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceCapturePolicy {
    Off,

    /// 只有整个 PreparedPlan 最终失败才持久化。
    FinalFailure,

    /// 每个 selector branch 失败都取证，即使之后 fallback 成功。
    BranchFailure,
}
```

开发 / E2E 推荐：

```text
BranchFailure
```

生产默认推荐：

```text
FinalFailure
```

并允许用户打开 BranchFailure。

为什么 BranchFailure 很重要：

```text
branch 0: accelerator_key = Ctrl+F   ← 已经坏了
branch 1: name starts_with 查找       ← 还能成功
```

如果只在最终失败取证，这个 regression 永远不会暴露。

---

## 9.3 Artifact kind

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceArtifactKind {
    PlannerExplain,
    ExecutionContext,
    SelectorTrace,

    UiaProcessTree,
    UiaCandidateSet,

    DomSnapshot,
    AxTree,

    Screenshot,
    OcrRegions,
    OcrOverlay,

    Logs,
}
```

---

## 9.4 Prepared diagnostics

```rust
#[async_trait]
pub trait PreparedDiagnostics: fmt::Debug + Send + Sync {
    fn backend(&self) -> BackendKind;

    async fn capture(
        &self,
        request: EvidenceCaptureRequest,
    ) -> Result<EvidenceBundle, EvidenceCaptureError>;
}
```

`PreparedCandidate`：

```rust
pub struct PreparedCandidate {
    explain: PlanExplain,
    execution: Arc<dyn PreparedExecution>,
    diagnostics: Option<Arc<dyn PreparedDiagnostics>>,
}
```

这里的 diagnostics 不是“重新分析原始 action”，而是绑定 prepare 阶段已经冻结的信息。

---

# 10. 为什么必须在 branch fallback 之前 capture

推荐 `PreparedPlan` 逻辑：

```text
candidate.execute()
    ↓ error
classify error
    ↓
trigger diagnostics for this frozen candidate
    ↓
persist / emit artifact refs
    ↓
根据现有 fallback 规则决定：
    BackendUnavailable → same-path backend fallback
    TargetNotFound     → later branch_path
    other              → abort
```

这个顺序有两个价值。

## 10.1 不让 fallback 擦掉失败现场

后一个 branch 可能改变 UI 状态，尤其是：

- 展开菜单；
- 切换 frame；
- OCR fallback 点击；
- SendInput 改焦点。

所以先 fallback 再 dump，证据可能已经不是失败时的现场。

---

## 10.2 能记录“失败但恢复”

Evidence Bundle manifest 可以有：

```json
{
  "outcome": "recovered_by_fallback",
  "failed_branch": [0],
  "recovered_branch": [1]
}
```

这对于持续维护自动化非常有价值。

---

# 11. Evidence Bundle 格式

推荐目录：

```text
.argusflow/
└── runs/
    └── <run_id>/
        └── evidence/
            └── <attempt_id>/
                ├── manifest.json
                ├── planner.json
                ├── context.json
                ├── selector_trace.json
                ├── summary.md
                ├── screenshot.png
                └── backend/
                    ├── uia/
                    │   ├── process_tree.json
                    │   ├── process_tree.txt
                    │   └── candidates.json
                    ├── cdp/
                    │   ├── dom_snapshot.json
                    │   └── ax_tree.json
                    └── ocr/
                        ├── regions.json
                        └── overlay.png
```

不要只生成一个巨大的 log。

### `manifest.json`

建议稳定 schema version：

```json
{
  "schema_version": 1,
  "run_id": "...",
  "node_id": "...",
  "backend": "windows_uia",
  "branch_path": [0, 1],
  "trigger": "target_not_found",
  "query": "menu_item(uia.accelerator_key=\"Ctrl+F\")",
  "artifacts": [
    {
      "kind": "uia_process_tree",
      "path": "backend/uia/process_tree.json"
    },
    {
      "kind": "screenshot",
      "path": "screenshot.png"
    }
  ]
}
```

路径只保存相对路径。

---

# 12. Selector Trace：比完整树更重要的诊断

单纯给开发者 500 个 UIA 节点，仍然需要人肉 grep。

真正有价值的是失败时自动回答：

```text
我在哪里找？
我看了多少候选？
每一步过滤掉多少？
最接近的是谁？
为什么它没通过？
```

推荐统一：

```rust
pub struct SelectorTrace {
    pub branch_path: BranchPath,
    pub scope: SelectorScopeSummary,
    pub stages: Vec<SelectorTraceStage>,
    pub near_misses: Vec<NearMiss>,
}
```

示例输出：

```text
Query:
  menu_item(uia.accelerator_key = "Ctrl+F")

Scope:
  Windows UIA / process_id=1234

Stages:
  Raw process candidates: 184
  ControlType=MenuItem: 17
  AcceleratorKey="Ctrl+F": 0

Near misses:
  #1
    Name: "查找(F)...\tCtrl+F"
    AutomationId: ""
    AcceleratorKey: ""
    AccessKey: ""
    Patterns: Invoke, LegacyIAccessible

Conclusion:
  Provider did not expose AcceleratorKey for the apparent Find menu item.
```

这时你根本不需要猜“为什么 Ctrl+F selector 不工作”。

---

# 13. Near Miss 必须是确定性解释，不是 fuzzy runtime selector

Near Miss 可以使用诊断评分排序，例如：

```text
role matched
parent relation matched
name contains target token
1 predicate failed
```

但它只能用于：

```text
Explain / diagnosis
```

不能用于：

```text
自动执行最高分候选
```

建议把两个类型完全分开：

```rust
ResolvedElement       // 执行用
DiagnosticCandidate   // 诊断用
```

避免未来无意间把 fuzzy 数据接进 action path。

---

# 14. UIA Failure Evidence 实现

建议新增：

```text
crates/argusflow-windows/src/uia/evidence.rs
crates/argusflow-windows/src/uia/selector_trace.rs
```

不要继续扩张 test helper。

## 14.1 Root 范围必须与 executor 一致

默认：

```text
CUIAutomation8.GetRootElement()
  ↓
ProcessId condition
  ↓
process-owned fragments
```

而不是：

```text
ElementFromHandle(main_hwnd)
  ↓
ControlView descendants only
```

---

## 14.2 推荐采集字段

每个 UIA node：

```text
runtime_id              // 只做诊断关联，绝不能当持久 selector
process_id
control_type
localized_control_type
name
automation_id
class_name
framework_id
accelerator_key
access_key
is_enabled
is_offscreen
has_keyboard_focus
bounding_rectangle
available_patterns
```

特别是：

```text
AcceleratorKey
AccessKey
FrameworkId
available_patterns
```

应加入现有 dump。

---

## 14.3 ControlView + RawView 分层采集

默认不要全量 RawView 无限制 dump。

推荐：

1. `process_tree.json`：有界 ControlView 全进程树；
2. `raw_neighborhood.json`：只对 near-miss / relation root 周边做 RawView；
3. `candidates.json`：真实 matcher 候选和逐 predicate 结果。

原因：

- ControlView 更易读；
- executor 的关系语义使用 RawView，诊断又不能完全缺 RawView；
- 全量 RawView 可能非常大且 provider traversal 昂贵。

---

## 14.4 继续使用预算模型

现有 `UiaExecutionBudget` 的思想应复用为：

```rust
pub struct EvidenceBudget {
    pub deadline: Duration,
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_bytes: usize,
    pub max_near_misses: usize,
}
```

建议诊断永远是 best-effort：

```text
主错误不能因为 dump 超时而被覆盖。
```

即：

```rust
match diagnostics.capture(...) {
    Ok(bundle) => attach(bundle),
    Err(capture_error) => log capture_error,
}

return original_automation_error;
```

---

# 15. CDP Failure Evidence

当前 `argusflow-browser` 仍是 compiler / plan 占位，但现在就应把未来诊断接口设计对。

CDP 不建议只输出：

```text
document.documentElement.outerHTML
```

因为 AQL 的浏览器语义未来很可能同时依赖 DOM 与 Accessibility Tree。

推荐 artifacts：

## 15.1 DOMSnapshot

调用：

```text
DOMSnapshot.captureSnapshot
```

它可以返回：

- 完整 DOM；
- iframe / template / flattened shadow DOM；
- layout；
- DOM rect；
- 可选 computed styles。

这是比手写 `outerHTML` 更适合诊断的结构化快照。

---

## 15.2 AX Tree

调用：

```text
Accessibility.getFullAXTree
```

因为 portable AQL 的：

```text
role
name
value
enabled
checked
selected
```

在浏览器侧与 AX Tree 高度相关。

如果 selector 在 AX compiler path 失败，只 dump DOM 是不完整的。

---

## 15.3 Screenshot

调用：

```text
Page.captureScreenshot
```

并记录：

```text
frame id
viewport
DPR / device scale
scroll position
active target/session
```

这样 DOM rectangle 才能和截图对齐。

---

# 16. OCR / Vision Failure Evidence

OCR 最容易犯的诊断错误是：

```text
识别失败
  ↓
重新截一张图
  ↓
保存第二张图
```

这样保存的并不是当时真正输入模型的帧。

正确做法：

> **持久化同一张 inference input frame。**

Evidence 应包括：

```text
input.png             // 真正参与推理的源帧
roi.png               // 若有裁剪
regions.json          // bbox / text / confidence
overlay.png            // bbox 可视化
preprocess.json        // resize / scale / threshold 等
model.json             // backend / model/version
```

如果视觉后端在内存里已有 frame handle / image buffer，PreparedDiagnostics 应持有只读引用或可持久化句柄，而不是失败后再调用 CapturePipeline。

---

# 17. Screenshot 应成为跨 backend 的公共辅助证据

虽然 UIA、CDP、OCR 的主事实源不同，但截图仍是非常有价值的公共 artifact：

```text
UIA tree     ↔ 用户真实看到什么
DOM / AX     ↔ 页面真实渲染什么
OCR boxes    ↔ 识别区域是否合理
```

因此 Evidence Collector 可以组合：

```text
backend-native evidence
+
optional screen evidence
```

但不要要求每个 backend 自己重新实现 Windows 截屏。

推荐后续在运行时装配一个共享：

```rust
ScreenEvidenceProvider
```

由 Windows capture 层实现。

---

# 18. 与 ExecutionEvent 集成

当前 `ExecutionEvent` 已有：

```text
run_id
workflow_id
node_id
sequence
payload
```

这是挂接 evidence refs 的天然位置。

建议新增：

```rust
ExecutionEventKind::DiagnosticEvidenceCaptured
```

以及：

```rust
ExecutionEventPayload::DiagnosticEvidenceCaptured {
    evidence_id: Uuid,
    backend: BackendKind,
    branch_path: Vec<usize>,
    recovered_by_fallback: bool,
}
```

事件里不要塞：

- 完整 UIA tree；
- screenshot bytes；
- DOM snapshot。

只放稳定引用和不敏感摘要。

大对象走 artifact store。

---

# 19. Artifact Store 不应塞进 backend

Backend 负责：

```text
capture evidence data
```

Runtime / host 负责：

```text
persist evidence artifact
```

推荐边界：

```text
PreparedDiagnostics
    ↓ EvidenceBundle (in-memory / temp handles)
EvidenceSink / ArtifactStore
    ↓
filesystem / future remote store
```

这样：

- UIA 不知道 `.argusflow/runs` 路径；
- Browser crate 不依赖 Tauri；
- Vision crate 不负责生命周期清理；
- test 可以替换成 InMemoryEvidenceSink。

符合仓库 AGENTS.md 的低耦合要求。

---

# 20. 隐私与脱敏

失败快照可能包含：

- 用户输入框文本；
- 浏览器 DOM 内容；
- 截图中的敏感信息；
- OCR 识别文本。

因此 Evidence 不能默认“无限、永久、全量”。

建议 policy：

```rust
pub struct EvidenceRetentionPolicy {
    pub persist_screenshot: bool,
    pub persist_text_values: bool,
    pub redact_password_controls: bool,
    pub max_total_bytes: usize,
    pub ttl: Option<Duration>,
}
```

默认至少：

- UIA password / protected control 不保存 Value；
- HTML password input value 不保存；
- screenshot 明确标记为 sensitive artifact；
- artifact UI 提供一键删除；
- CI 可配置不持久化截图，只存树和 selector trace。

---

# 21. Selector Stability Inspector：建议做，但不进入 runtime 自动选择

为了真正减少“今天能跑、明天 provider 一换就坏”，建议在开发工具里加入 selector inspector。

它读取当前候选元素的：

```text
role
name
automation id
accelerator key
access key
class/framework
parent relation
pattern capabilities
```

然后输出“建议”，例如：

```text
Recommended:
  menu_item(uia.accelerator_key = "Ctrl+F")

Fallback:
  menu_item(name starts_with "查找(F)...")

Warnings:
  AutomationId is empty in native CUIAutomation8
  Name contains localized mnemonic / accelerator suffix
```

可以有一个**仅用于作者体验**的稳定性评级：

```text
A: unique backend-native identity
B: semantic role + exact accessible name + relation
C: localized prefix / contains
D: ordinal / coordinate / volatile class
```

但评级只用于 lint / suggestion。

运行时仍然执行用户明确写下的 AQL。

---

# 22. 建议增加的 AQL / Planner 诊断

当前 `DiagnosticCode` 已有：

```text
BackendSpecificProperty
ResidualFilter
ExpensiveTraversal
PotentialMultiMatch
UnsupportedBackend
RuntimeUnavailable
```

可以补充偏开发期的：

```rust
FragileSelector,
RuntimePropertyUnavailable,
FallbackBranchRecovered,
```

但建议谨慎：

- `FragileSelector` 应来自 Inspector / lint，有明确规则才加；
- “某 provider 当前 AcceleratorKey 为空”属于 runtime evidence，不应冒充 compile-time AQL warning；
- `FallbackBranchRecovered` 更适合 ExecutionEvent / evidence manifest，不一定属于 Query Diagnostic。

所以 P0 不必急着扩 `DiagnosticCode`。

---

# 23. Notepad++ E2E 推荐重写

当前：

```text
menu_item(name = "搜索(S)")
sleep 300ms
menu_item(name starts_with "查找(F)...")
sleep 500ms
```

推荐演进为：

## 阶段 A：先扩大 dump 字段，实测 provider

失败 / inspector 输出：

```text
Name
AutomationId
AcceleratorKey
AccessKey
ClassName
FrameworkId
Patterns
RuntimeId
```

先确认 native CUIAutomation8 下的 Find 菜单项到底有什么稳定属性。

---

## 阶段 B：如果 `AcceleratorKey=Ctrl+F` 存在

改为：

```text
any(
    menu_item(uia.accelerator_key = "Ctrl+F"),
    menu_item(name starts_with "查找(F)...")
)
```

并给菜单出现使用 bounded wait，而不是 300ms 固定 sleep。

---

## 阶段 C：如果 AcceleratorKey 也为空

不要继续寻找“神奇数字 ID”。

使用明确组合：

```text
role + menu context + exact/prefix accessible name + action capability
```

同时让 failure evidence 记录这个 provider 的属性事实。

此时 prefix 仍然存在，但它变成：

> **已知 provider 限制下的显式 fallback**

而不是：

> **系统对 Find 菜单项唯一的识别方式**。

这两者的工程质量差别很大。

---

# 24. 推荐代码改动矩阵

## P0：先让问题“可见”

### `argusflow-windows/tests/support/uia_dump.rs`

短期增强：

- AcceleratorKey
- AccessKey
- FrameworkId
- RuntimeId
- pattern availability
- ProcessId
- BoundingRectangle

用于确认 Notepad++ 的真实 provider 属性。

### `argusflow-core/src/query.rs`

若实测有效：

```rust
UiaAttribute::AcceleratorKey
UiaAttribute::AccessKey
UiaAttribute::FrameworkId
```

优先只加真实用到的属性。

### `argusflow-windows/src/uia/native.rs`

增加对应强类型 property enum / projection。

### `argusflow-windows/src/uia/compiler.rs`

将新 UiaAttribute 映射为 pushdown / residual。

### `argusflow-windows/src/uia/property.rs`

支持读取 / residual 比较。

---

# 25. P1：正式 Failure Evidence

新增：

```text
crates/argusflow-agent/src/evidence.rs
```

包含：

- `EvidenceTrigger`
- `EvidenceCapturePolicy`
- `EvidenceArtifactKind`
- `EvidenceBundle`
- `PreparedDiagnostics`
- `EvidenceSink`

修改：

```text
crates/argusflow-agent/src/plan.rs
```

让 PreparedCandidate 可选携带 PreparedDiagnostics，并在 fallback 前触发 capture。

新增：

```text
crates/argusflow-windows/src/uia/evidence.rs
crates/argusflow-windows/src/uia/selector_trace.rs
```

并通过 UIA runtime worker 执行快照，保证 COM apartment 约束不被破坏。

> UIA COM element 仍然不能跨 runtime channel；跨线程返回的只能是普通 Rust snapshot DTO。

---

# 26. P2：等待与 Action-aware resolution

新增强类型：

```text
TargetWaitPolicy
ActionCapability
TargetResolutionFailure
```

修改 UIA executor：

```text
same compiled query polling
→ action suitability
→ unique resolution
→ action execute
```

证据在 wait deadline 后、fallback 前捕获。

---

# 27. P3：CDP / OCR 接入同一 Evidence 契约

CDP runtime 完成后，实现：

```text
PreparedCdpDiagnostics
├── DOMSnapshot.captureSnapshot
├── Accessibility.getFullAXTree
└── Page.captureScreenshot
```

Vision/OCR runtime 完成后，实现：

```text
PreparedVisionDiagnostics
├── original inference frame
├── ROI
├── detection / OCR regions
└── overlay
```

这时如果 `argusflow-agent::evidence` 被 UIA/CDP/Vision 三方稳定使用，再考虑抽成独立 crate。

不要提前为了“看起来架构完整”建一个只有 Trait 没有真实第二实现的新 crate。

---

# 28. 测试策略

## 28.1 UIA selector fallback 测试

故意写：

```text
any(
    menu_item(uia.accelerator_key = "__wrong__"),
    menu_item(name starts_with "查找(F)...")
)
```

期望：

```text
branch 0 → TargetNotFound
  ↓
evidence captured
  ↓
branch 1 → success
```

assert：

- workflow/action 最终成功；
- evidence manifest 标记 `recovered_by_fallback`；
- branch 0 的 selector trace 存在；
- tree snapshot 中可看到真实 Find item；
- evidence 不改变 fallback 排序。

这是最重要的新回归测试之一。

---

## 28.2 Evidence failure 不得覆盖业务错误

人为让 evidence collector 超时 / 失败。

期望原始：

```text
TargetNotFound
```

仍然返回 TargetNotFound，而不是：

```text
EvidenceCaptureError
```

---

## 28.3 PID fragment 测试

打开 popup menu / dialog 后：

- 主 HWND subtree 不包含目标；
- process-scoped evidence 仍包含目标；
- selector trace 的搜索 scope 与 executor 一致。

防止以后 diagnostics 又退化成主 HWND dump。

---

## 28.4 时序测试

让 menu/dialog 延迟出现：

```text
first resolve miss
second/third resolve success before deadline
```

期望：

- 不进入 fallback；
- 不提前 capture failure evidence；
- 同一 branch_path 保持不变。

---

# 29. 不建议做的方案

## 29.1 不重新注册 .NET ClientSideProviders 来“恢复 Item 43001”

这会让 ArgusFlow native UIA runtime 的行为依赖额外客户端代理配置，并把实现重新绑回特定 Win32 compatibility provider。

可以作为实验工具，但不应成为主架构。

---

## 29.2 不保存 RuntimeId 当 selector

RuntimeId 适合：

- 单次 snapshot 内关联；
- 去重；
- trace。

不适合：

- 持久 workflow；
- 跨运行 selector；
- 跨 provider/client 身份。

---

## 29.3 不用 `nth()` 修 AmbiguousTarget

除非业务语义真的就是“第 N 个”。

否则这是把歧义从运行时报错变成静默点错。

---

## 29.4 不在 backend executor 里硬编码应用特例

例如：

```rust
if process_name == "notepad++.exe" && name starts_with ...
```

不接受。

Notepad++ 应只是 E2E fixture，不是 UIA backend 的产品知识。

---

## 29.5 不把 screenshot / tree bytes 塞进 AutomationError

错误只应携带：

- 稳定分类；
- 必要摘要；
- 可选 evidence reference id。

大对象进入 artifact store。

---

# 30. 最终推荐架构图

```text
UiOperation / AutomationAction
          │
          ▼
        AQL
          │
          ▼
   Query Normalizer
          │
          ▼
  Backend Compiler
          │
          ├───────────────┐
          ▼               ▼
 PreparedCandidate    PlanExplain
          │
          ├── PreparedExecution
          │
          └── PreparedDiagnostics
                   │
                   │ frozen:
                   │ branch path / plan / PID-HWND / session / frame
                   │
          ▼
    PreparedPlan.execute
          │
          ▼
 execute same branch
          │
      ┌───┴─────────┐
      │ success     │ failure
      ▼             ▼
   outcome      classify trigger
                    │
                    ▼
             bounded diagnostics
                    │
                    ▼
              EvidenceBundle
                    │
                    ▼
               EvidenceSink
                    │
                    ▼
       emit artifact reference event
                    │
                    ▼
         existing fallback rules
       branch/backend deterministic
```

---

# 31. 我最推荐的落地顺序

如果按收益 / 风险比排序：

### 第一件事：扩 UIA inspector / dump 字段

先确认 Notepad++ native CUIAutomation8 对 Find MenuItem 是否给：

```text
AcceleratorKey=Ctrl+F
```

这是成本最低、最可能直接把 prefix 降级成 fallback 的改动。

### 第二件事：把固定 sleep 换成 PreparedPlan 内同分支 bounded wait

它不会改变 selector 语义，但会显著减少 timing noise。

### 第三件事：把 test-only UIA dump 升级成 process-scoped PreparedDiagnostics

先只做 UIA，真正跑通：

```text
branch failure
→ evidence
→ fallback success
```

### 第四件事：加 SelectorTrace / NearMiss

这会让“dump 了一堆树但还是不知道为什么失败”的问题真正消失。

### 第五件事：CDP runtime / OCR runtime 接入后复用同一 Evidence contract

此时抽象已经被至少两个真实 backend 验证，再决定是否拆 crate。

---

# 32. 对你这次兼容性问题的最终判断

你当前改动不是“绕路”，而是已经触碰到了真实 UI Automation 的边界：

```text
客户端代理差异
provider fragment 边界
Pattern 能力差异
Accessible Name 差异
时序
```

真正值得修的不是把：

```text
查找(F)...
```

再换成另一个看起来更硬的 magic string。

而是建立下面这套完整链路：

```text
稳定属性优先
  ↓
语义关系约束
  ↓
显式 any fallback
  ↓
同一 branch 有界等待
  ↓
严格唯一性 / action capability
  ↓
失败立即取证
  ↓
selector trace 告诉你为什么失败
```

这样将来 UIA、CDP、OCR 再遇到 provider / DOM / OCR 模型差异时，你不需要先猜“是不是又有某个 magic ID 失效”，而是直接打开一次 Execution Evidence，看到当时后端真实世界是什么样。

这比继续增加 selector 特例更符合 ArgusFlow 当前的 AQL + Planner + Backend Compiler 架构。

---

# 33. 参考依据

## 仓库文件

- `docs/ArgusFlow_AQL_统一UI查询语言设计方案.md`
- `docs/ArgusFlow_真实_UIA_对接方案_NotepadPP_E2E.md`
- `AGENTS.md`
- `crates/argusflow-core/src/query.rs`
- `crates/argusflow-core/src/automation.rs`
- `crates/argusflow-core/src/execution.rs`
- `crates/argusflow-query/src/alternative.rs`
- `crates/argusflow-query/src/diagnostic.rs`
- `crates/argusflow-agent/src/backend.rs`
- `crates/argusflow-agent/src/plan.rs`
- `crates/argusflow-agent/src/router.rs`
- `crates/argusflow-windows/src/uia/backend.rs`
- `crates/argusflow-windows/src/uia/compiler.rs`
- `crates/argusflow-windows/src/uia/executor.rs`
- `crates/argusflow-windows/src/uia/action.rs`
- `crates/argusflow-windows/tests/uia_notepadpp_e2e.rs`
- `crates/argusflow-windows/tests/support/uia_dump.rs`
- `crates/argusflow-browser/src/lib.rs`
- `crates/argusflow-vision/src/lib.rs`

## 外部规范 / API

- Microsoft UI Automation：Automation Element Properties / AcceleratorKey / AccessKey
  - https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-automation-element-propids
- Microsoft UI Automation：MenuItem Control Type
  - https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-supportmenuitemcontroltype
- Microsoft UI Automation：Using UI Automation for Automated Testing
  - https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-usefortesting
- Chrome DevTools Protocol：Accessibility Domain
  - https://chromedevtools.github.io/devtools-protocol/tot/Accessibility/
- Chrome DevTools Protocol：DOMSnapshot Domain
  - https://chromedevtools.github.io/devtools-protocol/tot/DOMSnapshot/
- Chrome DevTools Protocol：Page.captureScreenshot
  - https://chromedevtools.github.io/devtools-protocol/tot/Page/
