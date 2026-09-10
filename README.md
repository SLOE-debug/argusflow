# ArgusFlow

可独立调用的 Rust 自动化能力库，提供中文 AQL 编辑器与英文查询内核。当前 main 从空提交重新建立；experimental 保留旧实现供参考，未引入旧协议兼容层。

| crate | 能力 |
| --- | --- |
| `argusflow-core` | 几何、动作参数、总截止时间、协作取消、错误分类 |
| `argusflow-aql` | 英文查询模型、编译、类型绑定、树匹配、诊断与语言服务 |
| `argusflow-aql-wasm` | 中文标记转换、双向位置映射与编辑器 WASM 接口 |
| `argusflow-automation` | 显式绑定 UIA/DOM/OCR 来源的 Locator、点击和插入文字 |
| `argusflow-windows` | 窗口身份、专用 MTA UIA、独立真实输入服务、硬件 DXGI 桌面后端 |
| `argusflow-browser` | 持久 CDP 连接、浏览器进程所有权、AQL 与显式 iframe/Shadow 边界 |
| `argusflow-vision` | Rust 原生 PP-OCRv6 Small/Medium ONNX、稳定区域采样与结果复用 |
| `argusflow-capture-contracts` | 无平台和运行时依赖的来源、版本、只读像素、采样契约 |
| `argusflow-capture` | 共享采样、GPU 历史索引、变化订阅、时间锚点、稳定观察 |

中文编辑页使用 React/Vite/Monaco/Tailwind，通过本地 WASM 校验并导出英文 `.aql`。安装、启动和接口示例见 [AQL 说明](docs/aql.md)。当前不包含工作流、输入录制与持久化、整屏增量文字场景或 Tauri 装配。采样持续维护桌面状态，只有消费者需要视觉结果时才读回区域像素。

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
