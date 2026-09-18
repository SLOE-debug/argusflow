# ArgusFlow

本项目采用 [PolyForm Strict License 1.0.0](LICENSE) 官方原文，属于限制性源码许可。
允许协议规定的非商业用途；修改、二次开发、分发和商业使用需向相应权利人另行申请授权。
授权申请请通过原始仓库维护者联系渠道提出。第三方内容仍适用各自许可，此次变更不撤销此前已经有效授出的许可。

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

工作台使用 React/Vite/Monaco/Tailwind，支持无限画布、嵌套作用域缩放、复制粘贴、撤销重做、独立 workflow 调用、浅色/深色/系统主题，以及本地自动保存和运行结果查看。

中文 AQL 通过本地 WASM 检查并编译成英文查询。Rust 工作流引擎支持 Let 词法作用域、全局/局部变量、子流程、循环、异常处理和资源生命周期。输入录制与录制持久化、整屏增量文字场景尚未接入工作台。

开发启动：安装依赖后运行 `pnpm start`。生成桌面程序：`pnpm tauri build --no-bundle`，产物在 `target/release/argusflow.exe`。

`pnpm start` 直接启动 Tauri，保留 Rust 构建缓存；开发模式开启增量编译，图像处理等现有优化等级保持不变。缓存占用需要清理时，手动执行 `pnpm cache:prune`：仓库 `target` 超过 **10 GiB** 才执行 `cargo clean`，未超限保留缓存。预览用 `pnpm cache:prune -DryRun`，调整本次阈值用 `pnpm cache:prune -MaxGiB 40`。清理包含 release 可执行文件与增量结果，不删除 `.deps` 模型和 Cargo 下载缓存；清理后的首次构建会更慢。有 Cargo、rustc、rustdoc 或 ArgusFlow 进程时跳过清理，空闲后需重新执行命令。该阈值不是磁盘硬配额，仅管理默认 `target`。

开发服务器固定使用 `127.0.0.1:5173`，端口占用时直接报错，保持与 Tauri 的 `devUrl` 一致。Vite 仅从 `index.html` 扫描依赖，排除 Rust 源码、`target`、本地依赖缓存以及 `docs`、`scripts`、`tests` 目录的文件监听；Vitest 独立配置仍监听前端测试。Tailwind 从 `src` 与 `index.html` 检测类名，避免仓库构建产物拖慢前端启动。

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

源码按职责分层，测试统一位于根目录 `tests/<crate 名>/`，再分为 `unit/`、`integration/`、`support/` 和 `fixtures/`。目录约定见 [代理工作规则](AGENTS.md)。

编码与通用规则见 [docs/rules](docs/rules/)。
