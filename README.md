# ArgusFlow

ArgusFlow 是一个严格面向 Windows 的 Rust Agent Runtime 与可视化工作流设计器骨架。桌面端使用 Tauri 2，前端使用 React、TypeScript、Tailwind CSS 和 React Flow。

> ArgusFlow 只支持 `x86_64-pc-windows-msvc`。Cargo 已将该目标设为默认值，所有 Rust crate 在非 Windows 目标上都会直接拒绝编译；项目不提供 Linux、macOS 或移动端兼容层。

当前版本提供：

- 可序列化的工作流、RPA 动作和目标选择器契约；
- 线性工作流校验与 `Start → Log/Delay → End` 内存演示执行；
- UIA、CDP、视觉/OCR、GUI grounding 和 SendInput 的后端边界；
- 可编辑的节点画布、属性面板与实时运行日志。

完整功能状态和后续路线统一维护在 [TODO.md](TODO.md)。任何功能实现、删除或优先级调整都应同步更新该清单。

UIA、CDP、屏幕捕获和 OCR 暂未接入，相关调用会返回明确的 `BackendUnavailable` 错误。

## 目录

```text
argusflow/
├── crates/
│   ├── argusflow-core/
│   ├── argusflow-runtime/
│   ├── argusflow-agent/
│   ├── argusflow-windows/
│   ├── argusflow-browser/
│   └── argusflow-vision/
├── src-tauri/
├── src/
└── docs/
```

## 本地验证

项目要求 64 位 Windows 10/11、Rust 1.91+、Node.js、pnpm、Microsoft C++ Build Tools 和 WebView2。

一键启动 Tauri 与 Vite：

```powershell
pnpm start
```

首次安装依赖较慢时，可以让启动脚本使用 Clash 代理：

```powershell
.\scripts\dev.ps1 -Proxy http://127.0.0.1:7897
```

脚本会在 `node_modules` 不存在时执行冻结锁文件安装，然后由 Tauri 自动拉起 Vite 开发服务器。

完整验证命令：

```powershell
pnpm install
cargo test --workspace
pnpm test
pnpm tauri dev
```

这些编译和测试命令由项目维护者自行执行。
