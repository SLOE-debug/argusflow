# Rust Workflow

当前 workflow 使用独立的新版本契约，不加载 experimental 的历史文档。`argusflow-workflow` 拥有文档和数据类型，`argusflow-runtime` 编译与执行，`argusflow-workflow-automation` 装配实际能力。引擎不依赖平台、Tauri、浏览器协议或 OCR 实现。

## 公共入口

```rust
use argusflow_runtime::{prepare, NodeRegistry, WorkflowEngine, RunInputs, RunOptions};
use argusflow_workflow::Workflow;

async fn example(source: &str) -> Result<(), Box<dyn std::error::Error>> {
let definition = Workflow::from_json(source)?;
let registry = NodeRegistry::new();
let prepared = prepare(definition, &registry).map_err(|errors| format!("{errors:#?}"))?;
let engine = WorkflowEngine::new();
let mut run = engine.start(prepared, RunInputs::default(), RunOptions::default())?;
let _events = run.subscribe();
let result = run.wait().await?;
assert!(result.error.is_none());
Ok(())
}
```

`prepare` 不执行节点或外部 I/O。`start` 要求当前存在 Tokio runtime，并检查全部输入及资源端口。编译产物通过 `Arc` 共享，各次运行的变量、输出、取消和作用域实例彼此独立。默认一个引擎最多八个活动运行、六十四个自有资源；每次运行内部串行推进。

`RunHandle::cancel` 请求协作取消；丢弃句柄也取消，监督任务继续清理。`wait` 返回已完成收尾的结果；其自身的 `Err` 表示监督通道意外关闭，正常业务失败保存在 `RunResult::error`。事件携带运行 ID、作用域实例、节点执行序号及调用路径；订阅使用 256 条环形缓冲，消费落后返回 `EventRead::Gap`，最终状态和输出独立于事件可读。默认事件没有变量值、输入文字、图像和查询内容。显式打印最终输出或底层错误来源由宿主决定。

## 文档和结构图

文档包含 `schema_version: 1`、名称、根输入/输出类型、根借用资源、根作用域 ID、扁平 `scopes` 表和 `subflows` 表。JSON 使用 Serde 标记枚举，拒绝未知字段；`Workflow::from_json` 限制 8 MiB。公开 Rust 类型是唯一格式定义，示例的 `--json` 可以导出完整文档。

每个 Scope 有入口、节点和正常出口表达式。节点 `next` 只连接同一作用域中的节点；分支通过 If/Switch 拥有子作用域，返回容器后再执行 next。每个子作用域必须有唯一结构拥有者，子流程入口另由子流程定义拥有。拒绝循环连线、跨域边、重复 ID、不可达节点和共享子块；循环必须使用 While/ForEach。图的控制边不携带数据，数据通过表达式引用。

结构化分支没有任意跨块跳转或 fallthrough；If 的两块和 Switch 的默认块均显式存在，可使用空块。正常完成分支的容器输出类型必须一致；直接 Return、Fail、Break、Continue 的路径不参与正常出口类型合并。循环体与 Finally 不发布容器输出，跨轮累计值应声明在循环外。首期报告首个有作用域/节点位置的准备错误。

## 变量与词法作用域

根作用域的 Let 是本次运行全局变量。每个块、分支、循环体、Catch、Finally 和子流程激活拥有自己的帧。`Let` 声明名称、固定类型和初值，执行到声明节点才初始化。

- 同级同名声明报错；内层同名声明遮蔽外层。整个块内都绑定内层声明，声明前读写报暂时性死区错误，不回退读取外层。
- 没有内层同名声明时，读取和赋值沿词法链操作最近的外层声明。例：`let x = 1; { x = 2; }` 得到 2；内层改为 `let x = 2` 时外层保持 1。
- 输入、子流程参数、ForEach 的元素/索引、Catch 的错误记录只读。类型不随赋值改变；不存在隐式变量创建、隐式类型转换或共享可变对象别名。
- Assign 的全部右值求值成功并通过预算后一次提交，失败不部分写入。不同赋值项不能重复指向同一槽位。
- 每轮循环重建局部帧和节点输出，不读取上一轮结果。ForEach 进入时冻结集合，迭代元素来自该快照；While 在每轮开始前重新计算条件。
- 子流程的词法父级固定为根帧，可以读写已初始化的全局，不能看到调用方局部；局部数据和资源通过显式参数传入。每次调用有独立局部帧，返回值按值传递。准备阶段检查直接和传递的全局依赖，拒绝直接/间接递归。

节点输出仅在成功后发布，只有当前作用域已完成节点和可见祖先路径的节点可引用。子流程不捕获调用方节点输出。`output_bindings` 可读取当前任务的 `Result` 和可见变量，不能覆盖原生输出；映射整体成功才发布。节点已经产生的外部副作用不会因为映射错误回滚或重放。

## 类型与表达式

ValueType 支持 `bool`、`int`（i64）、`float`（有限 f64）、`text`、`list`、`record`、`optional`。Literal 必须携带类型，空列表及 Optional(None) 也无需猜测。资源不属于 Value，不能嵌入记录、全局变量或返回值。

Expr 支持变量、当前调用 Input、已完成 NodeOutput、映射阶段 Result、记录字段、整数索引、同类型算术与比较、短路 And/Or、Not、Choose、记录/列表构造、Some 和固定纯函数。整数溢出、除零、索引越界、非有限浮点结果均返回 Expression 错误。

函数有 Length、Concat、Contains、Trim、Lowercase、Uppercase、Split、Join、Append、IsSome、OrElse、ToFloat、ToText。OrElse 按需求值默认项；ToFloat 只接受绝对值不超过 2^53 的整数。AQL Number 使用 Float 端口，需要整数时由流程显式转换。没有脚本源码、动态 eval、I/O 或随机函数。

## 错误、取消与重试

Try 的 Catch 显式列出互不重复的 ErrorKind，绑定只读 `{kind: text, code: text}` 错误记录；仅 User 的 code 包含流程定义的错误代码。用户取消、运行总超时、引擎预算和扩展契约错误不可捕获。Break/Continue 只影响本次调用内最近的循环；Return 离开当前调用并按声明校验输出。

Finally 在正常、失败、跳转和取消退出时执行，使用独立的有界收尾操作；取消/致命展开共享总收尾预算。业务帧耗尽时仍预留最多 64 个收尾帧。Finally 不允许 Return 或跨块 Break/Continue 覆盖原始控制转移，但可跳出其内部新建的循环；最多执行一千个收尾节点。Finally 可以显式安排任务，因此宿主应把它用于预期的收尾行为。资源自动回收不依赖是否声明 Finally。

主错误保留发生位置和调用栈，后续收尾错误附加在 `secondary`；正常执行发生清理错误同样返回 Failed。最终状态为 Completed、Failed、Cancelled 或 TimedOut。原生调用只承诺协作取消，进程内引擎不声称能抢占卡死的原生调用或同步恶意插件。扩展 Future 的 panic 转为 Contract 并进入清理。

Retry 只作用于任务节点，默认关闭。最多十次（包含首次），有界指数退避，延迟最大 60 秒。启用时必须满足任务 `safe_to_retry`、没有资源创建输出、错误类别匹配、`Effect::None`、还有次数与总时限。节点超时覆盖所有尝试与退避，尝试不会重置父截止时间。点击、输入、导航、启动和附加资源不声明安全重试；成功后的输出映射不属于重试范围。

## 资源和能力装配

`register_automation` 使用 `AutomationHost` 注入共享 UIA/Input 服务和命名 QuerySource。OCR 来源由宿主显式提供 SampledOcr、采样来源、区域和窗口。流程不会重复初始化采样或 OCR 引擎。

任务通过 NodeCompiler 解码静态配置，PreparedTask 冻结端口并接收 TaskContext。Context 只有已求值输入、声明的资源端口和当前尝试票据，不暴露变量存储。扩展任务必须清理失败/取消前尚未交付的资源；成功交付后由引擎管理。Resource::cleanup 必须允许重复调用，以便显式重试失败清理。

资源由创建它的帧拥有。使用资源创建的新资源记录依赖，按逆序清理；父资源在依赖仍存活时不能关闭。Release 仅允许当前帧拥有的资源；子作用域借用的祖先资源和 RunInputs 的宿主资源不能关闭。局部创建资源不能通过子流程返回逃逸。自建资源在循环每轮退出时回收，清理失败后继续占资源额度，使用引擎 `retained_resources` 查询并在所有运行结束后调用 `retry_cleanup` 重试。

浏览器启动沿用现有独立 profile，附加已有浏览器只断开自己的连接；自建页面关闭并释放 context，附加页面只脱离 session。同一 BrowserResource 的相同 target 不允许重复获得清理所有权。宿主不能把同一外部连接包装成多个互相独立的所有者；跨运行复用时共享同一个 Arc<Resource>。

Windows Application 在 CREATE_SUSPENDED 后加入自有 Job，再恢复主线程；每棵进程树最多 128 个活动进程。关闭时先限制新增后代、保留成员的同步句柄，只终止本 Job，并等待活动计数归零及根进程、已观察成员句柄均报告退出。EXE 路径必须绝对且存在，argv 独立编码，不经过 Shell。首期定位根进程窗口，不自动认领单实例启动器转交后的外部窗口，也不按应用名字杀进程。Window.activate 显式请求前台，Windows 拒绝时直接报错，不绕过系统限制。实现依据 [Microsoft 进程列表](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_process_id_list)、[终止语义](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-terminatejobobject)与[进程数限制](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information)。

完整任务端口见 [节点目录](workflow-nodes.md)，验证及示例见 [验证记录](workflow-validation.md)。

## 默认预算

| 预算 | 默认 | 可配置上限 |
| --- | --- | --- |
| 单次运行时间 | 30 分钟 | 24 小时 |
| 执行步数 | 100000 | 10000000 |
| 单循环轮数 | 10000 | 100000 |
| 活跃帧深度 | 64 | 64 |
| 每批表达式操作数 | 10000 | 1000000 |
| 单值/单批输出映射 | 1 MiB | 64 MiB |
| 活跃数据总量 | 16 MiB | 256 MiB |
| 清理操作 | 5 秒 | 60 秒 |
| 事件缓冲 | 256 条 | 固定 |

文档最多 1024 个作用域、10000 个节点；类型深度 48，单表达式深度 48、最多 4096 个 AST 节点。数据计量保守地包含共享输入视图，不等同于进程实际内存。宿主应保持 Tokio runtime 存活到全部运行和清理完成。
