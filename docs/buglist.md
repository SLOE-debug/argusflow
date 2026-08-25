我重新按当前 `main` 审了一轮。最新提交已经变成：

**`bbd1a465adbad62ad7234ab572b28ec9a64a243f` — `fix: 修正 UIA 查询规划与执行边界`**。这个提交专门修了上一轮发现的 Any、动作能力、timeout/budget、HRESULT、前台激活和 E2E 等问题。

结论是：**上一轮的大部分问题已经修得比较扎实，但当时仍发现了 2 个 P0 查询规划问题。** 下文保留完整审计依据；截至 2026-08-26，本轮已修复除“任意 EXE + args 信任模型”之外的全部条目。该安全项按当前产品决策明确不改，不应被误标为已经解决。

## P0-1：`any(...)` 跨 backend 仍然会违反全局 branch 顺序

这次修复引入了：

```rust
earliest_supported_branch_index: usize
```

Router 也确实把它放到了 support/context/cost 之前排序。

UIA executor 也从之前的 union 改成了正确的：

```text
依次执行 branch
首个非空立即返回
```

这部分单 backend 语义已经正确。

但是问题在于：**一个 backend candidate 仍然可以同时携带多个不连续的原始 `any` branch。**

例如：

```text
any(
    button(uia.automation_id = "A"),  // branch 0
    button(dom.test_id = "B"),        // branch 1
    button(uia.automation_id = "C")   // branch 2
)
```

UIA compiler 会得到：

```text
UIA plan:
    branch 0
    branch 2

earliest_supported_branch_index = 0
```

因为 `compile_any()` 是把当前 backend 能编译的 branch 全部留下，然后丢掉不能编译的 branch。

CDP compiler 则得到：

```text
CDP plan:
    branch 1

earliest_supported_branch_index = 1
```

它也是相同的实现模式。

Router 因为：

```text
UIA earliest = 0
CDP earliest = 1
```

所以先执行 UIA。

但 UIA executor 内部会：

```text
尝试 branch 0
    ↓ empty
尝试 branch 2
    ↓ found
返回 branch 2
```

它**根本没有机会回到 Router 让 CDP 的 branch 1 先执行**。

结果实际顺序变成：

```text
0 → 2
```

正确语义应该是：

```text
0 → 1 → 2
```

这是确定性的语义 bug，不是性能问题。

这次新增的 router 测试没覆盖这个情况。测试 backend 每个 candidate 只有一个：

```rust
earliest_supported_branch_index
```

并没有模拟：

```text
同一个 backend 同时持有 branch 0 + branch 2
```

所以测试会绿，但真实 backend 仍然错。

### 更深的问题：一个 `usize` 不足以表达 AQL fallback algebra

尤其是嵌套：

```text
window(...)
>>
any(
    button(...),
    any(
        ...
    )
)
```

甚至：

```text
any(...) >> any(...)
```

当前 compiler 在 binary relation 里直接用：

```rust
max(
    left.earliest_supported_branch_index,
    right.earliest_supported_branch_index
)
```

来合并。

这实际上没有严格的查询代数含义。

一个 scalar：

```text
earliest_supported_branch_index: usize
```

只能解决最简单的“每个 backend 恰好只支持一个 top-level branch”。

### 推荐修法

不要继续给这个 scalar 打补丁。

应该把 `any` 真正提升到 Planner alternative：

```text
AQL

any(
  branch 0,
  branch 1,
  branch 2
)

        ↓

Planner alternatives

BranchPath [0]
    ├─ UIA candidate
    ├─ CDP candidate
    └─ Vision candidate

BranchPath [1]
    ├─ UIA candidate
    ├─ CDP candidate
    └─ Vision candidate

BranchPath [2]
    ...
```

排序规则：

```text
BranchPath
    ↓
SupportLevel
    ↓
ContextFitness
    ↓
Cost
    ↓
backend tie-break
```

对于嵌套 any：

```rust
pub struct BranchPath(Vec<usize>);
```

按 lexicographic ordering 即可。

关键原则是：

> **一个 PreparedCandidate 应对应一个明确 fallback alternative，而不是一个 backend 自己偷偷持有多个跨 backend 排序的 fallback branch。**

---

## P0-2：动作能力是在“整个 UIA any plan”上联合验证，导致可执行的前序 branch 被后序 unsupported branch 一起杀掉

这次新增的 `action_compiler.rs` 方向是对的：查询支持和动作支持现在真正联合计算了。

例如：

```text
checkbox()
+
Click
```

现在不会错误显示：

```text
Native Invoke
```

而是直接 Unsupported，这比上一版好很多。对应测试也有了。

但现在实现是：

```rust
collect_target_roles(&query.expression)
```

把**所有可能 branch 的最终角色全部收集起来**，然后：

```rust
try_fold(...)
```

只要任何一个 role 是 Unsupported：

```rust
Err(UiaActionCompileError::UnsupportedTargetRole)
```

于是整个 UIA backend candidate 被拒绝。

例如：

```text
any(
    button(name = "Save"),       // UIA Click 完全支持
    checkbox(name = "Remember")  // 当前 UIA Click 不支持
)
```

语义应该是：

```text
先找 Button

找到
→ UIA Invoke
→ 成功

Button 没找到
→ 才进入 checkbox fallback
```

当前实现却在 prepare 阶段看到：

```text
Button       Invoke = Native
CheckBox     Invoke = Unsupported
```

然后：

```text
整个 UiaBackend = Unsupported
```

于是即使界面上明明存在 `Save` Button，UIA 也完全没有资格执行 branch 0。

这又是一个直接功能 bug。

它和 P0-1 实际上是同一个架构问题的另一个表现：

> **Action capability 必须是 branch-specific，而不能是 backend-plan-global。**

如果采用前面说的：

```text
BranchAlternative
```

模型，这个问题自然消失：

```text
branch 0
    UIA query = supported
    UIA Click = Invoke Native
    → candidate exists

branch 1
    UIA query = supported
    UIA Click = unsupported
    → UIA candidate absent
```

而不是：

```text
UIA backend contains branch 0 + branch 1
↓
一个不支持
↓
整个 backend 被杀死
```

所以我会建议 **P0-1/P0-2 一起修，不要分别打补丁。**

---

## Security：`ApplicationQuery` 的本地进程启动信任边界仍然没有解决

这一项上一轮指出过，这次提交主要做的是**能力边界披露**，不是安全模型修复。

值得肯定的是，现在类型注释已经明确：

```text
direct-process Windows 桌面应用
不保证 bootstrapper / singleton handoff
UIA 不要求 foreground
```

这解决了“能力过度承诺”的问题。

foreground 也改成：

```text
Restore = required
SetForegroundWindow = best effort
```

这个修复是正确的。

但信任边界没有变化。

持久化 workflow 仍然可以携带：

```rust
ApplicationTarget {
    executable_path: String,
    arguments: Vec<String>,
}
```

Windows runtime 仍然直接：

```rust
Command::new(&executable_path)
    .args(&target.arguments)
    .spawn()
```

validator 只检查：

```text
absolute path
title 非空
timeout 范围
```

并没有：

```text
trusted application
user approval
workflow origin
allowed executable binding
```

之类的权限判断。

与此同时，Tauri privileged command 接受 renderer 直接传来的完整：

```rust
WorkflowDefinition
```

然后进入 WorkflowEngine。

因此安全边界仍然是：

```text
renderer / persisted workflow
       ↓
WorkflowDefinition
       ↓
ApplicationTarget
       ↓
任意绝对 EXE
+
任意 arguments
       ↓
Command::spawn
```

我不会把它描述成当前已经可远程利用的漏洞——目前没看到 workflow marketplace / 网络导入这种不可信入口。

但它是一个**必须在以下任何功能上线前关闭的高风险架构债务**：

```text
workflow 导入
workflow 分享
模板下载
插件
远程同步
第三方 workflow
任何 Web/Markdown 内容生成 workflow
```

更合理的最终模型依然是：

```text
workflow:
    ApplicationRef("notepad-plus-plus")

local trusted bindings:
    "notepad-plus-plus"
        →
    C:\Program Files\Notepad++\notepad++.exe
```

而不是让可分享 workflow 自带 EXE path。

我会把它定成：

**现在：P1 / security boundary disclosure**

**一旦引入不可信 workflow 来源：立即升级为 P0。**

---

## P1：新的 candidate budget 其实不是“硬查询预算”

这次新增：

```rust
max_candidates = 10_000
max_relation_roots = 256
```

整体方向很好。

但观察执行顺序：

```rust
let elements =
    root.FindAllBuildCache(...)?;

let length = elements.Length()?;

budget.observe_candidates(length)?;
```

也就是说 provider 已经完成：

```text
搜索整个 scope
构造完整 ElementArray
跨进程返回完整数组
```

之后才检查：

```text
是不是超过 10,000
```

所以：

```text
max_candidates
```

当前真正限制的是：

> ArgusFlow 后续读取多少已返回 candidate。

它**不能阻止 provider 为 100,000 / 1,000,000 个结果做完整 FindAll 和数组物化**。

因此不能把它当做：

```text
UIA provider traversal hard limit
```

它只是：

```text
post-materialization processing limit
```

好消息是这次同时配置了：

```text
IUIAutomation2 ConnectionTimeout
IUIAutomation2 TransactionTimeout
ArgusFlow execution timeout
```

所以最危险的永久 hang 已经显著改善。

但如果后续要真正防超大树，还是需要：

```text
TreeWalker / incremental traversal
或
有界 candidate enumeration
```

而不是 `FindAllBuildCache + length check`。

---

## P1：一次 execution timeout 会永久熔断当前 UIA runtime

当前 timeout 后：

```rust
health.mark_failed(...)
sender.send(Shutdown)
```

之后：

```rust
health.is_ready() == false
```

整个 `UiaRuntime` 不会再接收请求。

而且没有 restart/recovery generation。

所以一次偶发的：

```text
provider 卡顿 > 25s
```

会变成：

```text
UIA unavailable
直到 ArgusFlow 重启
```

这个选择在安全性上比“继续使用一个可能卡死的 COM worker”要好，所以它不是设计错误。

但产品上应该至少明确：

```text
UIA runtime tripped
Restart accessibility runtime
```

最终可以做：

```text
Generation 1 worker
    ↓ timeout
quarantine old worker

等旧 worker 已退出
    ↓
允许创建 Generation 2
```

不要在旧 worker 仍卡住时无限创建新 worker，否则会变成 thread leak factory。

我把它列 P1 resilience，而不是 P0。

---

## P1：`ApplicationQuery + BrowserCdp` 是合法序列化状态，但必然不可执行

当前：

```rust
TargetLocator::ApplicationQuery
```

是 core 的公开 locator。

但 CDP Backend 明确只接受：

```rust
TargetLocator::Query { query }
```

其它 locator 都 Unsupported。

Rust validator 又没有检查：

```text
ApplicationQuery
+
BackendPreference::BrowserCdp
```

这个组合。

所以以下对象可以成功序列化、成功进入 workflow：

```text
locator = application_query(...)
backend_preference = browser_cdp
```

最后运行时才：

```text
NoBackendAvailable
```

按照项目自己的强类型原则，这类非法组合最好在 validation 阶段直接拒绝。

P1 即可。

---

## P1：Windows executable path 的“大小写不敏感比较”只处理 ASCII

`WindowService` 现在：

```rust
left.to_string_lossy()
    .eq_ignore_ascii_case(...)
```

却注释为 Windows path case-insensitive comparison。

例如非 ASCII 安装路径：

```text
C:\工具\Notepad++\...
```

或者包含某些 Unicode case pair 的路径，行为并不等同 Windows 路径比较。

后果一般不是安全漏洞，而是：

```text
已有窗口无法识别
→ 又启动一个进程
→ duplicate application
```

建议以后用：

```text
Windows file identity
```

而不是字符串比较。

最稳妥的是：

```text
打开目标 executable
→ File ID / volume identity
```

与进程 image file identity 比较。

这一项优先级低于前面的几个。

---

## 已经修好的部分

上一轮这些问题我这次可以基本划掉：

* UIA `Any` 已提升为全局 `BranchPath` alternatives，不再由单个 backend 内部合并执行。
* query + action capability 已经开始联合计算，Checkbox Click 不再虚报 Native。
* UIA provider Connection/Transaction timeout 已显式配置。
* ArgusFlow request deadline 和资源 budget 已增加。
* Regex 已从 candidate 热路径移到 compiler/prepared plan 预编译。
* HRESULT 已经按 transient provider failure / implementation failure 分类，不再把 `E_INVALIDARG` 当 unavailable fallback。
* `SetForegroundWindow` 已经变成 best-effort。
* ApplicationSpec 已明确 direct-process contract。
* 已新增真正从 `WorkflowEngine → Application resource → Notepad++ → InvokePattern → 可观察 UI 状态` 的完整 E2E。
* 已新增普通 Windows CI 和 self-hosted interactive Notepad++ UIA workflow 配置。

不过当前 GitHub connector 没返回这个最新 SHA 的实际 status/check run，所以我只能确认 workflow 文件存在，**不能确认最新提交已经在 CI 上成功跑绿**。

## 本轮处理结果

我会把状态定成：

| 级别                          | 问题                                                                 | 处理结果 |
| --------------------------- | ------------------------------------------------------------------ | --- |
| **P0**                      | 一个 backend 持有多个不连续 `any` branch，导致跨 backend fallback 乱序            | **已修复**：compiler 展开独立 `BranchPath` candidate，Router 按完整路径字典序排序 |
| **P0**                      | action compiler 因后序 unsupported branch 拒绝整个 backend，包括可执行前序 branch | **已修复**：query/action capability 按 branch-specific plan 独立计算 |
| **Security P1 → future P0** | Workflow 可携带任意 EXE + args，Rust 端直接 spawn                           | **按要求不改**：继续作为已知产品边界保留 |
| **P1**                      | candidate budget 是 FindAll 后置预算，不是真正 provider traversal limit      | **已修复**：RawView TreeWalker 增量遍历，并在导航时执行硬节点预算 |
| **P1**                      | 单次 timeout 会永久熔断 UIA runtime                                       | **已修复**：引入 generation-aware、有恢复次数上限的 worker 替换 |
| **P1**                      | 应用资源作用域与 `BrowserCdp` 非法组合没在 validator 拦截                       | **已修复**：新增 `InvalidBackendPreference` 工作流校验 |
| **P2**                      | executable path 比较仅 ASCII case-insensitive                         | **已修复**：通过卷序列号与文件索引比较真实 file identity |

`earliest_supported_branch_index` 已从公共计划契约移除，当前边界调整为：

```text
Query Algebra
    ↓
Backend Compiler
    ↓
Branch-specific executable alternatives
    ↓
Global Planner
    ↓
PreparedCandidate
```

该边界使跨 UIA/CDP/Vision 的 fallback、Action capability、Explain 与 `TargetNotFound` 推进规则统一由同一条强类型路径约束。
