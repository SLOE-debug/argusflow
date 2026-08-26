继续审计了最新提交：

**`9d37937d46bb2ffaf1f9b6ef16f9e093506daa28` — `fix: 修复规划爆炸与命令进程树超时`**

结论：本轮列出的实现问题已经修复；编译与 CI 结果仍需由后续流水线确认。

本次修复同时把工作流契约升级到 schema v5。没有加入旧 schema、旧权限字段或旧启动接口的兼容分支。

以下各节保留修复前的审计证据和代码片段；节首“已修复”说明描述当前实现。

### 1. 已修复：Command 生命周期回归改用确定性 helper

原先依赖 `cmd.exe start /B` 的 fixture 已删除。集成测试现在直接再次启动当前测试二进制：根 helper 创建一个继承 stdout/stderr 的后代 helper 后立即退出，后代持续持有管道。该场景不再依赖交互 shell 或 GitHub runner 的 `start` 行为。

测试仍验证同一事实：根进程退出后，Job Object 必须终止后代、关闭继承的管道，并在 Command 自身 deadline 前返回。

最新 SHA 的 Windows CI 已经完成，`Rust compiler and runtime tests` 的 `Run workspace tests` 明确是 `failure`。

这次不是 BranchPath。前面的测试实际上已经证明：

* Router 跨 backend fallback 测试通过；
* CDP alternative 4096 hard limit 测试通过；
* query 层 checked overflow/budget 测试通过；
* AQL 测试全部通过。

真正失败的是新加的：

```text
command_finishes_after_killing_a_descendant_that_inherits_output_pipes
```

GitHub Windows runner 上实际结果：

```text
Command(Timeout { timeout_ms: 3000 })
```

约 3 秒后失败。

因此这次不能再说“Job Object 修复已经完成验证”。更准确的状态是：

> **实现方向正确，但专门用于证明这个问题已经修复的回归测试自己失败了。**

这里有两种可能：

1. 当前 `cmd / start /B` 测试 fixture 在 CI 的非交互 Windows 环境下并不具备你假设的“父进程立即退出”语义；
2. Job Object / suspended child / pipe 生命周期实现还有实际缺陷。

无论哪一个，都必须修。当前 CI 红灯足以阻止“通过”。

我建议不要继续拿 `cmd.exe start /B` 当核心生命周期证明。写一个非常小的 **test helper executable**：

```text
parent-helper.exe
    ├─ CreateProcess(child-helper.exe, inherit stdout/stderr)
    └─ 立即退出

child-helper.exe
    └─ 持有 stdout/stderr 并 sleep 20s
```

这样测试的事实完全由你控制：

```text
root exited
descendant still alive
descendant inherited pipes
ArgusFlow terminates job
pipes EOF
CommandExecutor returns before deadline
```

比 shell 行为稳定得多。

---

### 2. 上一轮的 alternative explosion：这项可以划掉

这次新增共享 `AlternativeExpansionBudget`，并且 UIA/CDP 都在：

```text
any:
    checked_add

relation Cartesian product:
    checked_mul
```

之后、真正分配之前检查 `4096` hard limit。

新 UIA 测试也覆盖了 13 个二选一关系产生 `8192` alternatives 时拒绝编译。

所以之前的：

```text
2^20 prepared alternatives
```

DoS 路径已经被实质封住。

这一项 **通过**。

---

### 3. BranchPath 契约也基本关闭

你现在正式选择了：

> `BranchPath` 描述 **normalize_query() 后的 AST**，而不是 source AST。

所以：

```text
any(
  A,
  any(B, C)
)
```

规范化成：

```text
any(A, B, C)
```

路径就是：

```text
[0]
[1]
[2]
```

UIA 若只支持 A/C：

```text
[0]
[2]
```

这和 normalizer 的 flatten 语义一致，之前 CI 失败的测试也已经改成这个契约。

`BranchPath` 类型本身也明确规定 compiler 必须一条 path 一个 candidate。

所以这项我也认为 **通过**。

---

### 4. 已修复：`WorkflowInput` 已拥有独立运行时输入

新增了持久化 `WorkflowInputDefinition`、瞬时 `RunInputs` 和文本输入类型。`WorkflowEngine::start`、Tauri `run_workflow` 与前端调用现在都显式传入本次运行输入。

`RunContext` 分别接收 `RunInputs.values` 与 `workflow.variables`，不再克隆同一对象充当两个数据面。Validator 只按输入声明校验 `WorkflowInput` 引用；启动时另行拒绝缺失、多余或类型不符的实际值。编辑器也分别提供“运行输入声明”和“不写回工作流的本次运行输入”区域。

这个是本轮新发现，而且属于比较核心的数据面契约问题。

`RunContext` 明确区分：

```text
workflow_inputs
variables
```

并把 `WorkflowInput` 描述成启动时冻结的输入。

但现在：

```rust
WorkflowEngine::start(
    workflow,
    sink,
)
```

根本没有 `inputs` 参数。

实际执行时做的是：

```rust
let inputs = workflow.variables.as_object().cloned()?;
let mut context = RunContext::new(run_id, inputs);
```

于是同一份持久化：

```text
workflow.variables
```

同时成为：

```text
WorkflowInput
+
Variable
```

Tauri 边界也是：

```rust
run_workflow(
    workflow: WorkflowDefinition
)
```

没有单独的运行参数。

validator 甚至要求 `WorkflowInput { key }` 必须存在于 `workflow.variables` 中。

也就是说现在：

```text
WorkflowInput
```

只是名字叫 input，本质上仍是 workflow JSON 里的静态变量。

这以后会直接阻塞：

```text
每次运行输入不同订单号
CLI/API 调用 workflow(params)
Credential / secret injection
Schedule trigger payload
Webhook payload
Parent workflow → subflow args
```

而且秘密数据如果想通过 `WorkflowInput` 使用，现在只能塞进持久化 workflow。

建议尽早改成：

```rust
pub struct RunInputs {
    pub values: Map<String, Value>,
}

WorkflowEngine::start(
    workflow,
    inputs: RunInputs,
    sink,
)
```

然后：

```text
WorkflowInput → RunInputs
Variable      → runtime variable store
workflow.variables → initial/default variables
```

Validator 应验证 **input schema/reference**，而不是验证当前持久化值一定存在。

我列 **P1**。

---

### 5. 已修复：Application 与 Command 使用统一能力边界

含义模糊的 `process_spawn` 已删除，替换为四个明确能力：

```text
application_launch
direct_command
powershell
cmd
```

Application 的 `attach_or_start` 与 `always_start_new` 必须声明 `application_launch`，`attach_only` 不需要启动能力；Validator 和节点执行器都执行该约束。三种 Command runner 分别只检查自身对应能力。

按既定范围，本次没有把绝对 EXE 改成 `ApplicationRef`，也没有实现宿主批准绑定；这不影响本轮修复 Application 绕过进程能力声明的问题。

这比之前“WorkflowPermissions 自我授权”又多一层。

当前权限：

```rust
WorkflowPermissions {
    process_spawn,
    powershell,
    cmd,
}
```

`process_spawn` 的注释是允许创建任意子进程。

Command validator 的确检查：

```rust
permissions.process_spawn
```

但 Application 节点走的是：

```rust
WorkflowNodeKind::Application { spec } => {
    validate_application_spec(...)
}
```

只检查：

```text
EXE absolute path
window title
launch timeout
```

**完全不检查 `process_spawn`。**

而 Application resource 本身在找不到进程时会启动它。

所以现在：

```text
permissions.process_spawn = false

Application {
    executable_path = "C:\\...\\anything.exe"
}
```

仍然可以产生新进程。

因此目前这个字段实际上不是：

```text
workflow may spawn processes
```

而只是：

```text
Command node may spawn processes
```

这两个语义最好不要混。

比较干净的模型是：

```text
RequestedCapabilities:
    application_launch
    direct_command
    powershell
    cmd

HostGrantedCapabilities:
    application bindings / allowed apps
    direct_command
    powershell
    cmd
```

尤其 Application 最终建议还是：

```text
ApplicationRef("notepad-plus-plus")
```

绑定本机批准的 EXE，而不是共享 workflow 自带绝对 EXE。

现在项目明确限定“可信本地 workflow”时，我仍然只定为 **P1 security architecture**；一旦有 import/share/marketplace/sync，它会升级。

---

### 6. 已修复：Command 输出超限立即终止进程树

stdout/stderr 读取任务现在发现第一个越界字节就返回 `OutputLimitExceeded`。Command 生命周期并发观察根进程和两个输出任务；任一输出超限都会立即触发 Job Object 终止，不再继续 drain 到进程自然退出。

新增的 helper 回归会持续输出数据，并验证节点在自身 deadline 之前返回精确的 stream/limit 错误。

现在：

```text
max_stdout_bytes
max_stderr_bytes
```

代码的做法大致是：

```text
达到 limit
→ 不再保留更多数据
→ 但继续 drain 管道
→ 等进程结束
→ 最后返回 OutputLimitExceeded
```

所以这个 limit 是：

> **内存 capture 上限**

而不是真正：

> **进程输出超过 limit 就立即终止**

但公开契约写的是“超限会终止节点”。

这意味着一个已获 Command 权限的程序可以在 timeout 内持续输出巨量数据，ArgusFlow 不会吃巨量内存，但仍会消耗 pipe I/O/CPU。

不是高危，我列 **P2**。要么改文档为 `capture_limit`，要么 read task 一检测 exceeded 就通知主任务 `TerminateJobObject`。

---

## 当前状态

| 项目                                  | 状态          |
| ----------------------------------- | ----------- |
| 跨 backend `any` 顺序                  | ✅           |
| branch-specific action capability   | ✅           |
| normalized BranchPath 契约            | ✅           |
| alternative 组合爆炸 hard limit         | ✅           |
| UIA TreeWalker hard budget          | ✅           |
| UIA generation recovery             | ✅           |
| Command Job Object 架构               | ✅           |
| Command descendant/pipe 确定性回归测试    | 🟡 已修复，待 CI |
| 真正的 runtime `WorkflowInput`         | ✅           |
| Application / Command 统一能力边界       | ✅           |
| output byte limit 精确语义              | ✅           |

所以这一轮的核心结论是：实现层面的四个未关闭项均已处理，剩余动作是由 Windows 编译和 CI 确认确定性 helper 回归及完整工作区测试。
