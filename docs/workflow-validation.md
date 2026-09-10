# Workflow 验证与示例

验证日期：2026-09-10，Windows x64。本次测试范围是 Rust workflow、能力票据入口、CDP 协议替身及隐藏的专属应用进程。没有运行真实浏览器、真实 UIA/键鼠或 OCR/GPU 验收。

## 执行结果

| 验证 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --workspace` | 143 通过、0 失败、11 ignored |
| runtime 显式目标 `workflow` | 43 通过 |
| automation 显式目标 `workflow_automation` | 6 通过、2 ignored |
| 单独执行上述 2 项应用进程验收 | 2 通过 |
| 三个示例 | 嵌套循环、子流程、协议替身加自有应用生命周期均通过 |

工作区测试的 11 个 ignored 中，2 个新增应用测试已显式执行。剩余 9 个属于已有真实浏览器、UIA/输入、采集和 OCR 原生验收，本次未执行。历史验证不作为本次实测结果。

## 覆盖内容

- 词法作用域：遮蔽、外层修改、暂时性死区、重复声明、全局初始化依赖、调用隔离、不可见兄弟输出与跨域连线。
- 类型与数据：严格类型、记录/列表/可选值、短路、溢出、赋值一致视图与原子提交、表达式预算、JSON 导入与静态示例执行。
- 结构控制：零次循环、每轮局部及输出重建、外层累计、Switch、最近循环 Break、Continue/Return 经过 Finally、递归拒绝、分支输出合并。
- 故障：局部超时可捕获、根时限不可捕获、祖先时限约束、取消时有界 Finally、帧耗尽后的收尾、首错及附加清理错误、编译和执行 panic、事件缺口与最终状态。
- 资源与副作用：安全读取重试、不确定副作用不重放、成功后映射失败不重放、契约错误仍回收新资源、逐轮回收、逆依赖顺序、宿主借用不关闭、超时清理保留额度与显式重试。
- 能力：共享票据连续调用、DOM AQL 快照及坐标、重复附加拒绝、自建页面关闭和外部页面脱离。浏览器验证全部使用本地 WebSocket 协议替身。

进程树验收启动隐藏夹具，夹具创建自己的后代并通过随机本地端口报告 PID。测试保留同步句柄，确认 shutdown 返回时后代已退出，另一棵同名外部夹具仍然存活。此测试发现并修复了仅检查 Job 活动计数可能过早完成清理的问题。

## 复现命令

在仓库根目录执行：

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cargo build -p argusflow-workflow-automation --example process_fixture
cargo test -p argusflow-workflow-automation --test workflow_automation application:: -- --ignored --test-threads=1

cargo run -p argusflow-runtime --example nested_loops
cargo run -p argusflow-runtime --example subflow
cargo run -p argusflow-workflow-automation --example automation_lifecycle -- --application "$PWD/target/debug/examples/process_fixture.exe"
```

第三个示例默认连接它自行创建的 CDP 协议替身；`--application` 演示自有应用的异常清理。不传该参数时只运行协议替身。只有显式传入 `--browser <EXE绝对路径>` 才启动真实浏览器，该路径本次没有执行。

## 流程文件

三个示例都支持 `--json`，只打印定义，不启动资源。固定文档可以直接通过 `Workflow::from_json` 加载：

- [嵌套循环与全局累计](../tests/argusflow-runtime/fixtures/nested-loops.json)：`total = 6`。
- [带参数的子流程](../tests/argusflow-runtime/fixtures/subflow.json)：`first = 2`、`second = 7`。
- [应用、浏览器与异常清理](../tests/argusflow-workflow-automation/fixtures/automation-lifecycle.json)：创建应用和浏览器，随后 Fail、Catch 和 Finally。EXE 使用 `C:/ArgusFlow/` 下的示例路径，实际运行前需替换。测试仅对此固定文件执行 prepare，执行示例使用专属进程和 CDP 替身。

全部测试、替身、夹具及可执行示例位于根目录 `tests/`，Cargo 显式注册。新增源码最大文件不足 500 行。模型、编译、表达式、执行数据、资源和平台适配分目录维护。

## 尚未实测的边界

真实 Chrome/Edge 完整 workflow、真实窗口等待与激活、UIA/OCR 查询点击输入、提升权限和多显示器场景留待用户验收。替身不能代替这些真实场景。引擎不抢占阻塞的原生调用或恶意同步插件；资源清理超时明确失败并保留占用，宿主需保持 runtime 和引擎存活以重试清理。
