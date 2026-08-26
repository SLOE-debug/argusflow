**本轮阻断项已完成修复。**

当前 `main` 最新提交仍是 **`3c0711661455168a148d86eecf9d0ddd3d484df1` — `fix: 修正查询分支规划与 UIA 执行边界`**。上一轮两个核心 UIA P0——跨 backend `any` 顺序和逐分支 Action capability——这次总体上已经按正确架构修掉了；UIA 的 TreeWalker 硬预算和分代 worker recovery 也明显改善。

但这轮仍有几个阻断项：

1. **当前最新提交 CI 是红的。** `Rust compiler and runtime tests` 的 `Run workspace tests` 明确失败，因此当前 SHA 本身不能判通过。
   失败点是新增的嵌套 `any` BranchPath 测试。这里还有一个契约冲突：测试要求 `any(A, any(B,C))` 保留源树路径 `[0] / [1,1]`，但现有 normalizer 明确定义为**扁平化嵌套 `any`**，因此编译器看到的是等价的 `any(A,B,C)`。
   这不一定意味着运行顺序已经错——扁平化仍可保持 A→B→C——但现在 **BranchPath 究竟描述 source AST 还是 normalized AST 没统一**。必须先明确契约并让 CI 通过。

2. **BranchPath 的正确语义修复引入了无上限的组合爆炸。** UIA/CDP 在关系表达式两侧都存在 alternatives 时直接做笛卡尔积；CDP 当前实现也明确如此。 AQL parser 对 `any` 分支数量和嵌套/关系深度没有相应上限。
   因而类似：

   ```text
   any(A0,A1) >> any(B0,B1) >> ... >> any(T0,T1)
   ```

   20 组就是约 **2²⁰ = 1,048,576 个 prepared alternatives**。这是实际的 CPU/内存 DoS 风险。建议 compiler 加 `max_alternatives` 硬预算并在乘法前 `checked_mul`，长期再考虑 lazy expansion。

3. **Command 节点的 `timeout_ms` 还没有覆盖完整命令生命周期。** 当前 deadline 包住 stdin 写入和 `child.wait()`，但主进程退出后，`stdout_task` / `stderr_task` 是无 deadline 的 `.await`；而超时时只 `kill()` 直接 child，没有 Windows Job Object/process-tree containment。
   如果命令生成一个继承 stdout/stderr 管道的后代进程，然后父进程先退出，`child.wait()` 会成功，但管道不会 EOF，随后 `join_output()` 可以无限等待。Node executor 外层也没有额外 timeout，会直接等待 CommandExecutor。
   建议把 **wait + 两个 pipe drain** 全部放入同一个 deadline，并在 Windows 下用 Job Object（`KILL_ON_JOB_CLOSE`）控制整个进程树。

还有一个安全边界需要继续保留在披露里：`WorkflowPermissions` 目前属于 workflow 自身可序列化、可在 UI 中开启的字段，runtime 直接信任这些布尔值；它更像“能力声明”，不是宿主独立授予的安全授权。  在全部 workflow 都是本机可信创建的前提下可以接受，但**在导入/分享/模板市场/远程 workflow 上线前必须拆成 RequestedPermissions 与 HostGrantedPermissions**。

因此本轮建议优先级是：**先修红 CI/明确 BranchPath 契约 → 给 alternative expansion 加硬预算 → 修 Command 全生命周期 timeout/process tree**。这三项完成并跑绿后，再做下一轮“是否通过”的审计。

## 本轮处理结果

1. **BranchPath 契约已统一。** `BranchPath` 明确描述 `normalize_query()` 输出的规范化 AST；嵌套 `any` 在保持顺序的扁平化后使用 `[0] / [1] / [2]`。原失败测试已经对齐为 UIA 实际支持的 `[0] / [2]`。
2. **替代方案展开已有硬预算。** query 层提供共享 `AlternativeExpansionBudget`，UIA/CDP compiler 的单个中间表达式最多物化 `4_096` 个 alternatives；`any` 累加和关系笛卡尔积都在分配前执行 checked arithmetic，溢出或超限返回结构化错误。
3. **Command deadline 已覆盖完整生命周期。** 根进程使用 `CREATE_SUSPENDED` 启动，在执行用户代码前纳入带 `KILL_ON_JOB_CLOSE` 的 Windows Job Object，再恢复主线程；stdin、根进程等待、后代清理和 stdout/stderr drain 共用同一 deadline。根进程退出、超时或 I/O 失败都会终止整个进程树。

`WorkflowPermissions` 的宿主授权边界本轮继续作为已知披露保留：当前仅适用于本机可信创建的 workflow；引入不可信来源前仍必须拆分 `RequestedPermissions` 与 `HostGrantedPermissions`。
