# ArgusFlow

可独立调用的 Rust 后端基础库。当前 main 从空提交重新建立；experimental 保留旧实现供参考，未引入旧协议兼容层。

| crate | 能力 |
| --- | --- |
| `argusflow-core` | 几何、动作参数、总截止时间、协作取消、错误分类 |
| `argusflow-windows` | 窗口身份、专用 MTA UIA、独立真实输入服务、硬件 DXGI 桌面后端 |
| `argusflow-browser` | 持久 CDP 连接、浏览器进程所有权、主文档页面/元素 |
| `argusflow-vision` | Rust 原生 PP-OCRv6 Small/Medium ONNX、稳定区域采样与结果复用 |
| `argusflow-capture-contracts` | 无平台和运行时依赖的来源、版本、只读像素、采样契约 |
| `argusflow-capture` | 共享采样、GPU 历史索引、变化订阅、时间锚点、稳定观察 |

不包含 AQL、工作流、输入录制与持久化、整屏增量文字场景、前端或 Tauri 装配。采样持续维护桌面状态，只有消费者需要视觉结果时才读回区域像素。

```powershell
cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

普通构建和测试不下载模型，也不需要 Python、CUDA 或 ONNX Runtime。真实 OCR 需要显式准备本地依赖：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/prepare-native-deps.ps1 -Device cpu
cargo run -p argusflow-vision --example recognize -- .deps tests/argusflow-vision/fixtures/bilingual.png cpu small
```

源码按职责分层，测试统一位于根目录 `tests/<crate 名>/`，再分为 `unit/`、`integration/`、`support/` 和 `fixtures/`。目录约定见 [项目结构](docs/layout.md)。

参见 [接口与生命周期](docs/backend.md)、[共享采样与录制器接入设计](docs/sampling.md)、[采样验证及性能报告](docs/sampling-validation.md)、[原生依赖与故障排查](docs/native-dependencies.md)、[验证记录与复现方法](docs/validation.md)。
