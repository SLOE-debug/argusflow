我按最新 `main` 审了一轮。当前最新提交是 **`b858ff4a8703dedc857441d7d0159302175cf31f` — `feat: 增加应用唤醒与 UIA 应用内查询`**，它是在真实 UIA 接入提交 `9aa9cf4...` 之后继续扩了 ApplicationQuery、窗口唤醒和 Notepad++ 生命周期。

结论先说：**UIA 的大方向没有重大错误**。专用 MTA COM worker、COM 对象不跨 Tokio 线程、HWND/PID 复验、native condition、CacheRequest、InvokePattern / ValuePattern 这些设计都是对的，而且微软官方也明确建议 UIA client 把查找和 pattern 调用放在独立 MTA 线程。([Microsoft Learn][1])

但是这一轮确实发现了 **2 个我认为必须按 P0 处理的问题，以及至少 4 个 P1 架构问题**。其中第一个还是原方案本身写错了。

### P0-1：`any(...)` 的语义现在是错的，而且方案和实现一起错了

这是目前最明确的逻辑 bug。

当前 AQL 核心契约已经写得很清楚：

> `Any` 是“多个可替代查询”，并且“**顺序代表回退优先级**”。

但我之前给出的 UIA 方案里写成了：

```text
for branch:
    execute branch
    append results
    dedupe

不要在第一个非空 branch 停止
```

也就是说把 `any` 当成了 **union/or result set**。

最新 executor 也照这个逻辑实现了：遍历全部 branch，然后 `append_unique` 合并所有结果。

这不是一个文字问题，会真实改变行为。比如：

```text
any(
    button(name = "Save"),
    button(name = "保存")
)
```

如果两个都存在，按照当前领域契约应该：

```text
branch 1 有结果
→ 使用 branch 1
→ 不再尝试 branch 2
```

现在却会：

```text
Save + 保存
→ 两个候选
→ AmbiguousTarget
```

更严重的是跨 backend：

```text
any(
    button(dom.test_id = "save"),
    button(uia.automation_id = "save")
)
```

如果第一个 branch 是 CDP、第二个是 UIA，branch 优先级本应让 CDP 优先。但目前 UIA compiler 会丢掉不支持的 DOM branch，而 Router 仍可能因为 UIA tie-break 先选 UIA，于是直接执行第二个 branch。

所以这个问题不能只把 executor 改成“first non-empty”就结束。

**正确修法应该是：**

```text
Any branch priority
        ↓
Backend compiler
        ↓
Candidate 携带 earliest_supported_branch_index
        ↓
Router ranking
        ↓
先比较 Any branch priority
再比较 capability/context/cost/backend tie-break
```

同时 UIA executor 内部：

```rust
for branch in branches {
    let result = execute(branch)?;
    if !result.is_empty() {
        return Ok(result);
    }
}
return Ok(vec![]);
```

注意如果第一个非空 branch 自己匹配了 3 个元素，仍然应该最终得到 `AmbiguousTarget`；不能因为 fallback 语义偷偷取第一个元素。

**这一项必须改方案正文第 22 节。**

---

### P0-2：最新 `ApplicationQuery` 实际上新增了“执行任意本地 EXE”的高权限能力，原方案完全没覆盖信任边界

最新 core 现在允许工作流保存：

```rust
ApplicationTarget {
    executable_path: String,
    arguments: Vec<String>,
    ...
}
```

而且是一个正式的持久化 `TargetLocator::ApplicationQuery`。

Windows 实现直接：

```rust
Command::new(&executable_path)
    .args(&target.arguments)
    .spawn()
```

也就是说，不经过 shell **并不能消除执行能力**；一个 workflow 完全可以配置一个系统 EXE 和任意参数。

同时 Tauri 的：

```rust
run_workflow(workflow: WorkflowDefinition)
```

会直接接收 renderer 给出的完整工作流并进入 runtime。

所以这项能力意味着：

```text
Workflow
    ↓
ApplicationTarget
    ↓
任意绝对 EXE + arguments
    ↓
用户权限下的本地进程启动
```

如果 ArgusFlow 将来支持：

```text
导入 workflow
分享 workflow
云同步 workflow
模板市场
外部文件打开
```

那么 workflow 本身就已经接近“可执行内容”，不能再只按普通 JSON 配置看待。

即便目前只有用户自己手工编辑，这也应该在架构里作为 **privileged capability** 明确披露。否则以后加 workflow import 时很容易漏掉。

我建议不要让可分享 workflow 直接成为：

```text
C:\xxx\foo.exe + arguments
```

更稳的模型是：

```text
Workflow
    ApplicationRef("notepad-plus-plus")
             ↓
Local Application Binding
    用户机器上批准：
    notepad-plus-plus
        → C:\Program Files\Notepad++\notepad++.exe
```

并且：

```text
首次导入/首次运行未知 ApplicationRef
→ 明确用户确认

修改 executable/arguments
→ 重新确认

信任判断必须在 Rust/Tauri privileged boundary
→ 不能只靠 React UI
```

是否进一步限制 PowerShell/cmd/mshta/rundll32 等属于产品安全策略，但**至少要先有“工作流应用启动权限”这层模型**。

这部分是最新提交引入的，所以不是原 UIA 实现写坏，而是**方案已经过期，需要新增一个完整章节**。

---

### P1-1：UIA worker 虽然线程模型正确，但缺少执行预算和 timeout 策略

现在：

```rust
response_receiver.await
```

没有 ArgusFlow 自己的 deadline。

UIA worker 又是单线程串行：

```text
request 1
request 2
request 3
...
```

只要一个 provider 调用长期卡住，后面的所有 UIA 请求都会排队。并且 `Drop` 会：

```rust
worker.join()
```

无上限等待。

这里其实不需要自己从零发明 timeout。Windows UIA 已经提供 `IUIAutomation2`：

* Connection timeout 默认约 **2 秒**
* Transaction timeout 默认约 **20 秒**
* 两个值都可以配置。([Microsoft Learn][2])

UIA 也正式定义了：

```text
UIA_E_TIMEOUT
UIA_E_NOTSUPPORTED
UIA_E_ELEMENTNOTAVAILABLE
...
```

这些应该进入错误分类。([Microsoft Learn][3])

所以我建议 `UiaRuntime` 初始化时直接拿：

```rust
IUIAutomation2
```

并显式配置：

```text
connection_timeout
transaction_timeout
```

不要依赖系统默认值。

同时再增加 ArgusFlow 层：

```rust
UiaExecutionBudget {
    deadline,
    max_candidates,
    max_relation_roots,
}
```

否则：

```text
textbox()
text()
pane()
```

在某些复杂应用里可能构造非常大的 `FindAllBuildCache` 数组。

微软官方也明确警告过 UIA 大范围 descendants 查询可能遍历数千元素；当前是 HWND subtree，比 desktop root 安全很多，但仍然需要 budget。([Microsoft Learn][4])

---

### P1-2：现在的 `SupportLevel::Native` 只证明“能找到”，没有真正证明“这个 Action 能执行”

原方案其实已经意识到一半问题：

> query compiler 只能证明元素可查，最终实例是否有 InvokePattern / ValuePattern 要运行时检查。

但当前实现还是：

```rust
AutomationAction::Click
    -> UiaActionPlan::Invoke

AutomationAction::SetValue
    -> UiaActionPlan::SetValue
```

无条件映射。

而：

```rust
PlanExplain.support
```

仍直接取：

```rust
query_plan.capability.level
```

也就是说：

```text
checkbox(name="Enable")
Click
```

完全可能 Explain：

```text
WindowsUia
Native
Ready
Action: InvokePattern
```

然后执行时：

```text
RequiredPatternUnavailable
```

因为 Checkbox 正常语义一般是 Toggle，而不是 Invoke。

类似的还有：

```text
Radio        → SelectionItem
ListItem     → SelectionItem
ComboBox     → ExpandCollapse
某些 MenuItem → Invoke 或 ExpandCollapse
```

所以长期不能维持：

```text
Click == InvokePattern
```

作为所有 UIA control 的语义。

这里建议增加：

```rust
pub enum UiaActionSupport {
    Native,
    RequiresRuntimePatternCheck,
    Unsupported,
}
```

以及：

```rust
pub struct UiaPreparedPlan {
    query: UiaQueryPlan,
    action: UiaActionPlan,
    capability: UiaCapabilitySummary,
}
```

最终 `PlanExplain.support` 应该是：

```text
query capability
+
action capability
```

共同产生。

如果暂时不准备支持 Toggle / SelectionItem / ExpandCollapse，那么 P0 最安全的办法反而是**明确限制 Click 可接受的 target role**，不要对 checkbox/radio 等错误报告 Native。

---

### P1-3：`AutomationError` 的 fallback 分类现在太粗，会在未来掩盖 UIA 实现 bug

当前代码大致把：

```text
GetPattern / Invoke / SetValue
→ BackendFailed
```

其他大多数 UIA native error：

```text
CreateCondition
BuildCache
FindAll
ReadProperty
ElementFromHandle
...
→ BackendUnavailable
```

而项目的 PreparedPlan 有个非常重要的规则：

```text
只有 BackendUnavailable 才允许 fallback
```

所以将来 Vision/OCR/SendInput 真正可用后，如果：

```text
UIA CreateCondition 因为代码 bug 返回 E_INVALIDARG
```

当前分类有可能变成：

```text
UIA BackendUnavailable
    ↓
偷偷 fallback Vision/其它 backend
    ↓
动作反而执行了
```

这会把确定性的实现 bug 隐藏掉。

应该根据 **HRESULT + operation** 分类，而不能主要根据“发生在哪个阶段”。

例如：

```text
UIA_E_TIMEOUT
RPC/provider unavailable
window 已消失
→ BackendUnavailable

UIA_E_NOTSUPPORTED
UIA_E_INVALIDOPERATION
E_INVALIDARG
property type mismatch
compiler/native IR 不一致
→ BackendFailed

UIA_E_ELEMENTNOTAVAILABLE
→ stale/retry policy

0 candidates
→ TargetNotFound

>1 candidates
→ AmbiguousTarget
```

这一项建议在其它 fallback backend 真正接入前修，否则以后非常难查错。

---

### P1-4：`ApplicationTarget` 当前宣称得比真实能力宽

目前公共类型叫：

```text
ApplicationTarget
```

看起来是通用 Windows 应用定位。

但当前启动后查窗口有三个实际限制。

第一，workflow 必须持久化**绝对 EXE 路径**。

这意味着：

```text
我的机器:
C:\Program Files\Notepad++\notepad++.exe

另一台机器:
D:\Portable\Notepad++\notepad++.exe
```

同一个 workflow 就不能直接复用。

所以它和 AQL 追求的 portable intent 有点冲突。最好把：

```text
Application identity
```

和：

```text
本机 executable binding
```

拆开。

第二，启动应用后只接受：

```text
window PID == Command::spawn() 得到的 PID
```

这对 Notepad++ `-multiInst` 很稳定，但很多 Windows 应用存在：

```text
bootstrapper.exe
      ↓
真实 app.exe

或者

新进程启动
      ↓
把请求交给已有 singleton process
      ↓
新进程退出
```

这种情况下现在会一直等不到窗口。

所以如果暂时只服务 Notepad++/普通 direct-process Win32 app，应该在方案里明确写：

> P0 ApplicationTarget 只保证 direct-process desktop applications。

否则公共契约表达得太强。

第三，代码把 `SetForegroundWindow` 成功当成硬条件；Windows 如果拒绝把窗口切到前台，整个 ApplicationQuery 就 `BackendFailed`。

但 UIA 本身很多情况下**不要求窗口成为 foreground**就可以查询、Invoke、SetValue。

所以建议分成：

```text
EnsureRunning
EnsureRestored
BestEffortForeground
```

UIA 只要求前两个。

真正需要 physical input 的 SendInput backend 才要求：

```text
ForegroundRequired
```

不然 UIA 会被 Windows foreground-lock 机制无意义地拖累。

---

### P1-5：真实 E2E 有了，但“产品完整路径”仍然没真正闭环

这一块实现已经比方案预期好很多。

现在有一个真实 Notepad++ UIA E2E，覆盖：

```text
Window
Search
Find
SetValue
AmbiguousTarget
TargetNotFound
Regex residual/cache
Close
```

而且明确断言 `ActionOutcome.backend == WindowsUia`，这很好。

另外最新提交又有：

```text
ApplicationQuery
→ 启动 Notepad++
→ UIA Invoke
```

测试。

但这里还有两个验证缺口：

1. 两组真实测试全部是 `#[ignore]`，不会进入普通 test。
2. `application_query_launches_notepadpp_before_uia_invoke` 只验证 ActionOutcome 成功，没有像 Find dialog E2E 那样验证**可观察 UI 状态真的改变**。

而我查最新 commit 时 GitHub combined status 目前也是空的，没有看到对应 Windows UIA CI gate。

所以发布前最好至少做到：

```text
普通 CI:
    compiler/unit/router/runtime tests

Interactive Windows runner:
    Notepad++ real UIA E2E

至少一条:
    WorkflowDefinition
      ↓
    WorkflowEngine
      ↓
    ApplicationQuery
      ↓
    启动真实 Notepad++
      ↓
    UIA action
      ↓
    可观察 UI 状态断言
```

最好是自托管交互式 Windows VM，避免 GUI/UIA 测试依赖普通无桌面 CI 环境。

---

### 还有两个次一级问题

一个是 regex residual 目前会对**每一个 candidate 重新 `RegexBuilder::build()`**，能改成 query/request 级预编译；另一个是 `FindAllBuildCache` 没有 candidate 数量预算。这两个还不至于推翻设计，但建议和 timeout/budget 一起收掉。

综合下来，我会把审计状态定成：

| 级别     | 问题                                               | 是否需要改方案         |
| ------ | ------------------------------------------------ | --------------- |
| **P0** | `any()` fallback 语义被写成 union                     | **必须，原方案本身错误**  |
| **P0** | ApplicationQuery 任意 EXE/args 的信任模型缺失             | **必须，最新提交新增边界** |
| **P1** | UIA timeout / execution budget 缺失                | 必须补             |
| **P1** | Query Support ≠ Action Support                   | 必须补             |
| **P1** | HRESULT/fallback 分类太粗                            | 必须补             |
| **P1** | ApplicationTarget 过度承诺、绝对路径/child PID/foreground | 必须补             |
| **P1** | 真机 E2E 没进入持续验证                                   | 实施流程需补          |

所以现在**不建议继续往 Scintilla、更多 Pattern 或 OCR fallback 扩功能**。先把 `Any` 语义和 ApplicationQuery 信任边界两个 P0 定下来，再补 UIA timeout/action capability/error taxonomy。否则后面 backend 一多，返工会明显变大。

[1]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-threading?utm_source=chatgpt.com "Understanding Threading Issues - Win32 apps | Microsoft Learn"
[2]: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation2-get_connectiontimeout?utm_source=chatgpt.com "IUIAutomation2::get_ConnectionTimeout (uiautomationclient.h) - Win32 apps | Microsoft Learn"
[3]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-error-codes?utm_source=chatgpt.com "Error Codes (UIAutomationCoreApi.h) - Win32 apps | Microsoft Learn"
[4]: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-findall?utm_source=chatgpt.com "IUIAutomationElement::FindAll (uiautomationclient.h) - Win32 apps | Microsoft Learn"
