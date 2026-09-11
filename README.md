# ArgusFlow

Tauri 工作流设计器与可独立调用的 Rust 自动化能力库，提供结构化画布、中文 AQL 编辑和真实运行。当前 main 使用新的强类型工作流契约；experimental 保留旧实现供参考，未引入旧协议兼容层。

| crate | 能力 |
| --- | --- |
| `argusflow-core` | 几何、动作参数、总截止时间、协作取消、错误分类 |
| `argusflow-aql` | 英文查询模型、编译、类型绑定、树匹配、诊断与语言服务 |
| `argusflow-aql-wasm` | 中文标记转换、双向位置映射与编辑器 WASM 接口 |
| `argusflow-automation` | 显式绑定 UIA/DOM/OCR 来源的 Locator、点击和插入文字 |
| `argusflow-workflow` | 强类型流程文档、表达式树、词法作用域与结构化控制流 |
| `argusflow-runtime` | 准备校验、隔离执行帧、取消重试、事件与资源回收 |
| `argusflow-workflow-automation` | AQL 任务、浏览器和 Windows 应用生命周期适配 |
| `argusflow-windows` | 窗口身份、专用 MTA UIA、独立真实输入服务、硬件 DXGI 桌面后端 |
| `argusflow-browser` | 持久 CDP 连接、浏览器进程所有权、AQL 与显式 iframe/Shadow 边界 |
| `argusflow-vision` | Rust 原生 PP-OCRv6 Small/Medium ONNX、稳定区域采样与结果复用 |
| `argusflow-capture-contracts` | 无平台和运行时依赖的来源、版本、只读像素、采样契约 |
| `argusflow-capture` | 共享采样、GPU 历史索引、变化订阅、时间锚点、稳定观察 |
| `argusflow-desktop` | Tauri 工作台、工作目录、自动保存、运行管理与有序日志 |

工作台使用 React/Vite/Monaco/Tailwind，支持无限画布、嵌套作用域缩放、复制粘贴、撤销重做、独立 workflow 调用、浅色/深色/系统主题，以及本地自动保存和运行结果查看。启动与操作见 [设计器使用说明](docs/workflow-designer.md)，本轮测试范围见 [设计器验证记录](docs/workflow-designer-validation.md)。

中文 AQL 通过本地 WASM 检查并编译成英文查询，语言接口见 [AQL 说明](docs/aql.md)。Rust 工作流引擎支持 Let 词法作用域、全局/局部变量、子流程、循环、异常处理和资源生命周期，见 [Workflow 使用说明](docs/workflow.md)。输入录制与录制持久化、整屏增量文字场景尚未接入工作台。

开发启动：安装依赖后运行 `pnpm start`。生成桌面程序：`pnpm tauri build --no-bundle`，产物在 `target/release/argusflow.exe`。

开发服务器固定使用 `127.0.0.1:5173`，端口占用时直接报错，保持与 Tauri 的 `devUrl` 一致。Vite 仅从 `index.html` 扫描依赖，排除 Rust 源码、`target` 和本地依赖缓存目录的文件监听；Tailwind 从 `src` 与 `index.html` 检测类名，避免仓库构建产物拖慢前端启动。

```powershell
cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm test
pnpm typecheck
pnpm build
```

普通构建和测试不下载模型，也不需要 Python、CUDA 或 ONNX Runtime。真实 OCR 需要显式准备本地依赖：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/prepare-native-deps.ps1 -Device cpu
cargo run -p argusflow-vision --example recognize -- .deps tests/argusflow-vision/fixtures/bilingual.png cpu small
```

源码按职责分层，测试统一位于根目录 `tests/<crate 名>/`，再分为 `unit/`、`integration/`、`support/` 和 `fixtures/`。目录约定见 [项目结构](docs/layout.md)。

参见 [接口与生命周期](docs/backend.md)、[共享采样与录制器接入设计](docs/sampling.md)、[采样验证及性能报告](docs/sampling-validation.md)、[原生依赖与故障排查](docs/native-dependencies.md)、[验证记录与复现方法](docs/validation.md)。
