# ArgusFlow

ArgusFlow 是一个严格面向 Windows 的 Rust Agent Runtime 与可视化工作流设计器骨架。桌面端使用 Tauri 2，前端使用 React、TypeScript、Tailwind CSS、Zustand 和项目内自研 Flow 引擎。

> ArgusFlow 桌面运行时只支持 `x86_64-pc-windows-msvc`。`argusflow-core` 与 `argusflow-query` 保持跨平台，以便同一套 AQL 语言引擎编译到 WebAssembly；其余运行时与后端不提供 Linux、macOS 或移动端兼容层。

当前版本提供：

- 可序列化的工作流、RPA 动作和目标选择器契约；
- 条件 DAG 工作流校验、完整 JSON 变量比较与单路径模拟执行；
- UIA、CDP、视觉/OCR、GUI grounding 和 SendInput 的后端边界；
- 支持框选、快捷编辑、对齐分布、吸附辅助线、缩放平移、四向锚点、端点重连和正交避障的自研节点画布；
- Start、End、Log、Delay、Condition 多形态节点、连线流动粒子、属性面板与实时运行日志。

完整功能状态和后续路线统一维护在 [TODO.md](TODO.md)。任何功能实现、删除或优先级调整都应同步更新该清单。

UIA、CDP、按窗口捕获和本地 PaddleOCR worker 已接入；视觉动作只处理当前 AppSession
窗口的短期像素，不会默认捕获整个桌面或把截图发送到云端。

## 目录

```text
argusflow/
├── crates/
│   ├── argusflow-core/
│   ├── argusflow-runtime/
│   ├── argusflow-agent/
│   ├── argusflow-query/
│   ├── argusflow-query-wasm/
│   ├── argusflow-windows/
│   ├── argusflow-browser/
│   └── argusflow-vision/
├── src-tauri/
├── src/
└── docs/
```

## 本地验证

项目要求 64 位 Windows 10/11、Rust 1.91+、Node.js、pnpm、Miniconda/Anaconda、
Microsoft C++ Build Tools 和 WebView2。启动前需保证 `conda` 命令可用；Vision worker
使用的 Python 3.11 由启动脚本在仓库专属环境中创建，不读取 Conda base 环境的包。

AQL Editor 使用项目自研的 Rust/WASM 语言服务。首次启动或修改 `argusflow-query` 后，先安装 WASM target 与版本匹配的 `wasm-bindgen-cli`，再生成 Vite 静态资源：

```powershell
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127
pnpm build:aql-wasm
```

生成物位于 `src/features/aql-editor/generated`，属于本地构建产物，不提交到仓库，并由 Vite 作为源码模块处理。

一键启动 Tauri 与 Vite：

```powershell
pnpm start
```

首次安装依赖较慢时，可以让启动脚本使用 Clash 代理：

```powershell
.\scripts\dev.ps1 -Proxy http://127.0.0.1:7897
```

脚本会按需安装冻结的前端依赖，并在 `workers/argusflow-vision-worker/.conda` 中创建
仓库独享的 Conda Python 3.11 prefix 环境。PaddleOCR 依赖和模型首次安装、预热完成后，
脚本才会启动 Tauri 与 Vite；退出桌面端时，本次启动的 worker 会一并结束。已经由外部部署层设置
`ARGUSFLOW_VISION_PIPE_NAME` 与 `ARGUSFLOW_VISION_SESSION_TOKEN` 时，脚本会沿用外部 worker。
需要执行视觉工作流时不要绕过该脚本直接运行 `pnpm tauri dev`，除非外部 worker 已配置完成。

完整验证命令：

```powershell
pnpm install
cargo test --workspace
pnpm test
pnpm start
```

这些编译和测试命令由项目维护者自行执行。
