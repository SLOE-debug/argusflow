# ArgusFlow 功能清单

> 长期维护的项目功能与路线图。最后更新：2026-08-20。
>
> ArgusFlow 严格限定为 64 位 Windows，目标为 `x86_64-pc-windows-msvc`；不规划 Linux、macOS 或移动端支持。

## 维护规则

- `[x]` 表示功能、测试和必要文档均已落地；仅有接口或空实现时必须标注“骨架”，不能把真实能力勾选为完成。
- 每个功能使用稳定编号。实现、重构或删除功能时，在同一次变更中同步更新本清单。
- `P0` 是当前主路径，`P1` 是下一阶段能力，`P2` 是增强项；优先级变化时直接修改对应条目。
- 新功能完成前至少具备结构化错误处理和针对核心行为的测试；不以 panic、假执行或静默回退代替实现。
- 与当前目标契约不一致时直接更新到新方案，不添加未被明确要求的兼容分支。

## 当前里程碑：M0 可运行架构

### 工程基础

- [x] `FOUNDATION-001` `P0` 建立 Rust 2024 Cargo workspace 与职责独立的 crate。
- [x] `FOUNDATION-002` `P0` 项目、crate、二进制和窗口统一使用 ArgusFlow 命名。
- [x] `FOUNDATION-003` `P0` 将默认编译目标固定为 `x86_64-pc-windows-msvc`。
- [x] `FOUNDATION-004` `P0` 所有 Rust crate 在非 Windows 目标上直接拒绝编译。
- [x] `FOUNDATION-005` `P0` 生成并维护 `Cargo.lock` 与 `pnpm-lock.yaml`。
- [x] `FOUNDATION-006` `P0` 提供 PowerShell 一键开发启动脚本及可选 Clash 代理。
- [ ] `FOUNDATION-007` `P1` 建立统一日志、配置加载和应用数据目录约定。
- [ ] `FOUNDATION-008` `P1` 建立 CI：格式、静态检查、测试和 Windows 构建。
- [ ] `FOUNDATION-009` `P2` 建立依赖更新、安全审计和许可证检查流程。

### 核心契约

- [x] `CORE-001` `P0` 定义 schema v2 `WorkflowDefinition`、JSON 变量、节点、分支连线和画布位置。
- [x] `CORE-002` `P0` 定义 Start、Log、Delay、Action、End 节点契约。
- [x] `CORE-003` `P0` 定义 Click 与 SetValue 自动化动作契约。
- [x] `CORE-004` `P0` 定义 Native、Browser、VisualText、Coordinate 选择器。
- [x] `CORE-005` `P0` 定义 backend 类型、动作结果和结构化自动化错误。
- [x] `CORE-006` `P0` 定义运行 ID、事件序号、节点状态和工作流状态事件。
- [ ] `CORE-007` `P1` 增加窗口、进程和应用范围的目标约束。
- [ ] `CORE-008` `P1` 增加观察结果、元素快照、矩形和置信度契约。
- [ ] `CORE-009` `P1` 增加节点输入/输出端口及类型化数据传递契约。
- [x] `CORE-010` `P2` 增加结构化 JSON 条件、True/False 分支与只读变量契约。
- [ ] `CORE-012` `P2` 增加循环、变量写入和受控表达式契约。
- [ ] `CORE-011` `P2` 建立 Rust/TypeScript 契约自动生成，消除手工镜像。

### 工作流运行时

- [x] `RUNTIME-001` `P0` 校验 schema 版本、名称、节点和连线 ID。
- [x] `RUNTIME-002` `P0` 校验唯一 Start/End、边端点、自环、环路和可达性。
- [x] `RUNTIME-003` `P0` 校验无环条件 DAG，并保证每次运行只选择一条命中路径。
- [x] `RUNTIME-004` `P0` 异步执行 Start、Log、Delay 和 End。
- [x] `RUNTIME-005` `P0` 发送有序运行事件并拒绝重复并发启动。
- [x] `RUNTIME-006` `P0` Action 未接入时返回明确失败事件。
- [ ] `RUNTIME-007` `P1` 支持取消当前运行与应用退出时的安全清理。
- [ ] `RUNTIME-008` `P1` 支持节点超时、重试、失败策略和执行上下文。
- [ ] `RUNTIME-009` `P1` 支持运行快照、历史记录和失败现场保存。
- [x] `RUNTIME-010` `P2` 支持条件 DAG、JSON 谓词、分支汇合和连线流转事件。
- [ ] `RUNTIME-012` `P2` 支持循环和并行节点。
- [ ] `RUNTIME-011` `P2` 支持断点、单步执行和从指定节点重放。

### Tauri 桌面端

- [x] `DESKTOP-001` `P0` 建立 Tauri 2 Windows 桌面应用壳。
- [x] `DESKTOP-002` `P0` 实现 validate_workflow 与 run_workflow IPC。
- [x] `DESKTOP-003` `P0` 将 Rust 运行事件桥接到主窗口。
- [x] `DESKTOP-004` `P0` capability 仅允许 Windows 主窗口所需命令和事件权限。
- [ ] `DESKTOP-005` `P1` 增加应用级设置页面和运行环境诊断。
- [ ] `DESKTOP-006` `P1` 增加窗口关闭前的未保存工作流提示。
- [ ] `DESKTOP-007` `P2` 增加托盘、通知和全局快捷键。

### 可视化工作流编辑器

- [x] `EDITOR-001` `P0` 集成 React、TypeScript、Vite、Tailwind CSS、Zustand 和项目内自研 Flow 引擎。
- [x] `EDITOR-002` `P0` 提供 Start、Log、Delay、Condition、End 节点注册表和空白初始文档。
- [x] `EDITOR-003` `P0` 支持多形态节点新增、删除、流畅拖动和属性编辑。
- [x] `EDITOR-004` `P0` 支持四向锚点连线、选择、删除、端点重连和条件分支标签。
- [x] `EDITOR-005` `P0` 展示 Rust 校验问题并标记对应节点。
- [x] `EDITOR-006` `P0` 展示实时执行日志与节点运行状态。
- [ ] `EDITOR-007` `P1` 支持工作流新建、打开、保存、另存为和最近文件。
- [ ] `EDITOR-008` `P1` 增加 Click、SetValue、Wait、窗口和浏览器节点面板。
- [x] `EDITOR-009` `P1` 增加撤销/重做、复制/粘贴、重复、多选、框选和桌面快捷键。
- [x] `EDITOR-010` `P1` 增加对齐、等距分布、自动吸附、辅助线、节点搜索和缩放平移。
- [x] `EDITOR-014` `P1` 增加圆角正交自动避障、Worker 精确路由、视口裁剪和运行粒子流动。
- [ ] `EDITOR-011` `P1` 增加节点级错误、耗时、输入和输出检查器。
- [ ] `EDITOR-012` `P2` 增加子流程、节点分组、注释和可复用模板。
- [ ] `EDITOR-013` `P2` 增加运行时间线、断点和单步调试界面。

## M1 Windows 原生自动化主路径

### UI Automation

- [x] `UIA-001` `P0` 建立 `UiaBackend`、模块边界和未接入错误骨架。
- [ ] `UIA-002` `P0` 初始化 COM apartment 与 `IUIAutomation` 生命周期。
- [ ] `UIA-003` `P0` 实现桌面、窗口和元素查找。
- [ ] `UIA-004` `P0` 将 Native selector 转换为 UIA condition。
- [ ] `UIA-005` `P0` 实现 Name、AutomationId、ControlType 等常用属性匹配。
- [ ] `UIA-006` `P0` 实现 CacheRequest 批量属性和 pattern 预取。
- [ ] `UIA-007` `P0` 实现 Invoke、Value、SelectionItem、Toggle 和 ExpandCollapse pattern。
- [ ] `UIA-008` `P1` 实现 StructureChanged、FocusChanged 和属性事件订阅。
- [ ] `UIA-009` `P1` 实现事件处理组、缓存失效与元素重新定位。
- [ ] `UIA-010` `P1` 建立 UIA 专用线程、COM 线程边界和异步任务桥接。
- [ ] `UIA-011` `P2` 增加 LegacyIAccessible pattern 兜底。

### 窗口与输入

- [x] `WIN32-001` `P0` 建立窗口服务和 SendInput backend 骨架。
- [ ] `WIN32-002` `P0` 枚举顶层窗口并获取进程、标题、类名和边界。
- [ ] `WIN32-003` `P0` 实现窗口激活、恢复、最小化和前台校验。
- [ ] `WIN32-004` `P1` 实现多显示器、DPI 与逻辑/物理坐标转换。
- [ ] `INPUT-001` `P1` 实现鼠标移动、单击、双击、滚轮和拖拽。
- [ ] `INPUT-002` `P1` 实现键盘按键、组合键和 Unicode 文本输入。
- [ ] `INPUT-003` `P1` 增加输入前目标窗口确认和执行后状态验证。

## M2 Chromium 自动化路径

- [x] `CDP-001` `P0` 建立 `CdpBackend` 和未接入错误骨架。
- [ ] `CDP-002` `P0` 发现 Windows 上开启调试端口的 Chrome、Edge 和 Chromium。
- [ ] `CDP-003` `P0` 建立 CDP WebSocket 会话、请求 ID 和事件分发。
- [ ] `CDP-004` `P0` 实现 target、frame、DOM 和 execution context 管理。
- [ ] `CDP-005` `P0` 实现 CSS selector 查询、点击和文本赋值。
- [ ] `CDP-006` `P1` 接入 Accessibility tree 并缓存语义节点。
- [ ] `CDP-007` `P1` 处理页面导航、frame 重建、弹窗和会话断线。
- [ ] `CDP-008` `P1` 实现 DOM/AX 命中失败后的坐标输入兜底。
- [ ] `CDP-009` `P2` 支持显式启动并管理专用 Chromium 实例。

## M3 Windows 视觉路径

### 截图与图像管线

- [x] `CAPTURE-001` `P0` 建立 DXGI 与 Windows.Graphics.Capture 模块骨架。
- [ ] `CAPTURE-002` `P0` 实现 DXGI Desktop Duplication 屏幕捕获。
- [ ] `CAPTURE-003` `P0` 实现 WGC 指定窗口捕获。
- [ ] `CAPTURE-004` `P0` 实现 D3D11 texture 生命周期、帧同步和错误恢复。
- [ ] `CAPTURE-005` `P1` 提取 dirty rect 与 move rect，仅更新变化区域。
- [ ] `CAPTURE-006` `P1` 根据 UIA/CDP 语义边界计算 ROI。
- [ ] `CAPTURE-007` `P1` 实现 GPU crop、resize 和颜色格式转换。
- [ ] `CAPTURE-008` `P2` 支持多显示器、HDR 和显示模式变化恢复。

### OCR 与视觉树

- [x] `VISION-001` `P0` 建立视觉缓存、OCR tiny/medium 与 grounding backend 骨架。
- [ ] `VISION-002` `P0` 选定并记录 PP-OCRv6 tiny/medium 模型来源与许可证。
- [ ] `VISION-003` `P0` 建立模型下载、校验、版本和本地缓存机制。
- [ ] `VISION-004` `P0` 接入 ONNX Runtime 并实现 CPU execution provider。
- [ ] `VISION-005` `P1` 接入 TensorRT、CUDA 与 WinML execution provider。
- [ ] `VISION-006` `P1` 启动时探测并选择可用推理 backend。
- [ ] `VISION-007` `P0` 实现检测、识别、置信度和文本框结果解析。
- [ ] `VISION-008` `P1` 实现 tiny 快路径与 medium 低置信度回退。
- [ ] `VISION-009` `P1` 建立 persistent visual tree 和 dirty rect 增量更新。
- [ ] `VISION-010` `P1` 实现视觉文本选择器匹配和坐标动作输出。
- [ ] `VISION-011` `P2` 接入 GUI grounding/VLM，并限制为最终慢路径。

## M4 Agent 编排与恢复

- [x] `AGENT-001` `P0` 固定 UIA → CDP →视觉缓存 → OCR tiny → OCR medium → GUI grounding → SendInput 路由顺序。
- [x] `AGENT-002` `P0` 建立统一 `ActionBackend` 接口和未接入 backend 跳过逻辑。
- [ ] `AGENT-003` `P0` 根据 selector、应用类型和缓存状态选择可用 backend。
- [ ] `AGENT-004` `P1` 为每次动作记录尝试路径、耗时、结果和失败原因。
- [ ] `AGENT-005` `P1` 实现动作后置验证和错误恢复策略。
- [ ] `AGENT-006` `P1` 实现缓存元素失效后的重新 grounding。
- [ ] `AGENT-007` `P2` 实现 planner、工具调用约束和结构化执行计划。
- [ ] `AGENT-008` `P2` 实现恢复预算、人工确认和高风险动作拦截。

## M5 数据、质量与发布

### 持久化与数据管理

- [ ] `DATA-001` `P1` 定义 `.argusflow.json` 工作流文件格式。
- [ ] `DATA-002` `P1` 实现原子保存、损坏文件错误和最近文件列表。
- [ ] `DATA-003` `P1` 保存运行记录、节点结果、截图引用和诊断信息。
- [ ] `DATA-004` `P2` 支持敏感字段标记、凭据引用和日志脱敏。
- [ ] `DATA-005` `P2` 支持工作流模板导入与导出。

### 测试与性能

- [x] `TEST-001` `P0` 覆盖核心 JSON 契约往返测试。
- [x] `TEST-002` `P0` 覆盖线性工作流校验、事件顺序和重复运行拒绝。
- [x] `TEST-003` `P0` 覆盖未接入 Action 的失败事件。
- [x] `TEST-004` `P0` 覆盖前端契约映射、错误归一化和事件状态展示。
- [ ] `TEST-005` `P1` 建立可重复的 Windows UIA 测试应用和集成测试。
- [ ] `TEST-006` `P1` 建立 Chromium CDP 测试页面和集成测试。
- [ ] `TEST-007` `P1` 建立截图/OCR 固定数据集和回归基线。
- [ ] `PERF-001` `P1` 建立 UIA 查找、CDP 操作、截图和 OCR benchmark。
- [ ] `PERF-002` `P1` 记录端到端动作 P50/P95 延迟与内存/GPU 占用。
- [ ] `PERF-003` `P2` 验证 DXGI → GPU crop → OCR 推理的数据零拷贝路径。

### 安全、诊断与发布

- [ ] `SECURITY-001` `P1` 为高风险输入、进程和窗口操作增加显式权限策略。
- [ ] `SECURITY-002` `P1` 审核 Tauri CSP、capability 和 IPC 输入边界。
- [ ] `OBS-001` `P1` 增加结构化日志、trace ID 和可导出的诊断包。
- [ ] `OBS-002` `P1` 在 UI 中展示 backend 状态、模型状态和环境检查结果。
- [x] `RELEASE-001` `P1` 生成 ArgusFlow 图标、版本信息和 Windows 资源。
- [ ] `RELEASE-002` `P1` 配置 NSIS 安装包与 WebView2 安装策略。
- [ ] `RELEASE-003` `P1` 配置代码签名、自动更新和发布说明。
- [ ] `RELEASE-004` `P2` 建立 Windows 安装、升级、卸载和离线环境测试。

## 明确不在范围内

- [x] 不支持 Linux、macOS、Android 或 iOS。
- [x] 不为旧工作流字段主动增加兼容映射、双写或回退逻辑。
- [x] 不使用第三方 RPA wrapper 替代 Windows UI Automation 原生接口。
- [x] 不把 VLM 作为常规动作主路径。
