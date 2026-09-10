# AQL 首期验证记录

日期：2026-09-10。工作区 Windows 桌面，显示器 2560×1600、125% 缩放。全部测试源码均位于根目录 `tests/`。

后续编辑器修复：补载 Monaco 原生 hover、候选和片段模块；修正英文前缀被中文标签二次过滤，补齐函数、属性与参数说明。按用户要求保留原有 300ms hover 延迟，不做切换延迟优化。本次相关 Rust 测试 15 项、前端测试 14 项通过，类型检查、生产构建、格式检查与工作区 Clippy 通过；Monaco 模块装配在 jsdom 中验证，没有启动浏览器。下表是首期完整验收记录。

## 结果

| 检查 | 结果 |
| --- | --- |
| `cargo check --workspace --all-targets` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --workspace` | 86 通过，0 失败，9 默认 ignored |
| `pnpm typecheck` | 通过 |
| `pnpm test` | 11 通过，包含真实 WASM 与组件测试 |
| `pnpm build` | 通过，生成 WASM 与静态编辑页 |
| UIA AQL 专属窗口验收 | 1 通过，显式运行 |
| 原有 UIA Pattern / 真实键鼠验收 | 1 通过，显式运行 |
| Small/Medium CPU 固定图片及 AQL 查询 | 1 通过，显式运行 |
| Small/Medium CPU/CUDA 对照及 AQL 查询 | 1 通过，显式运行 |

工作区普通 Rust 测试分布：AQL 6、中文转换 5、automation 3、browser 24、capture 19、capture-contracts 2、core 2、vision 11、windows 14，共 86。默认 ignored 共 9 项，其中本次单独执行 4 项通过；其余 5 项为真实浏览器 2 项、GPU 差分/性能 2 项和 DXGI 采样 1 项。本次按约定不启动浏览器，未重复本次未涉及的 GPU/DXGI 验收。历史原生结果见 [基础验证记录](validation.md)，不计作本次运行。

生产构建仍提示 Monaco 动态模块超过 500 kB；该模块已经按需加载，构建正常。Windows 本地 cdylib 链接打印导入库生成信息；原生窗口测试环境打印 libpng iCCP 配置警告，均未导致测试失败。

## 覆盖内容

- 语言：布尔优先级、分组、直接子级与后代、全局先序去重、first/nth、参数冻结和类型错误、正则、缺失值语义、预算和取消。
- 中文：完整词汇往返、字符串/正则/CSS/注释/参数保护、格式化幂等、中文/emoji/CRLF 的双向范围、未完成草稿不生成英文导出。
- 编辑器：真实 WASM 序列化、UTF-16 诊断和补全范围、输入法草稿、过期分析丢弃、导入编码/大小、迟到导入不能覆盖新编辑、有效导出、格式化回写。
- 动作：每次重新定位、零/多匹配不注入、取消和非法文字不执行、部分失败不重放、同一 Operation 身份贯穿查找和动作、输入序列独占。
- CDP 替身：AX 名称与角色、Rust 正则、document/frame/Shadow 嵌套、普通后代不穿透边界、同进程/跨进程 frame、子会话输入路由、导航失效、带边框/缩放/负坐标投影、拒绝非轴对齐变换、远端资源释放和超时副作用状态。
- 固定 DOM 脚本：输入收起选区且不改写原值、只读拒绝、先序属性读取不穿透 Shadow Root。此项使用 jsdom 单元测试，不启动浏览器。
- 原生窗口：UIA 值/勾选属性及缺失 Pattern、释放句柄失效、物理 DPI 矩形与实际 HWND 一致、左键单击、已有中文处于选中状态时追加文字不覆盖。
- 原生 OCR：固定双语 PNG 及旋转/透明变体，Small/Medium CPU/CUDA 一致性；类型化中文参数、文字与置信度过滤、阅读顺序、屏幕四边形映射、采样内容复验及来源撤销拒绝旧坐标。

原生验收暴露的 DPI 偏移已修复：UIA MTA 原先使用逻辑坐标，125% 缩放下与真实输入不一致；现在 worker 生命周期内使用物理 DPI。输入选区验收同时验证已聚焦控件不重复 SetFocus，防止提供者重置光标。

CUDA 首次运行因 DLL 搜索路径冲突尝试加载 `cublasLt64_11.dll` 并退出失败；按仓库已有命令将准备好的 CUDA 12 目录置于当前进程 PATH 最前后，CPU/CUDA 对照重跑通过，没有引入代码回退或下载新依赖。

## 复现

先按 [AQL 安装说明](aql.md) 准备 WASM 工具链与前端依赖。

```powershell
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm test
pnpm typecheck
pnpm build
```

原生窗口命令会短暂操作专属窗口的鼠标和键盘，依次运行：

```powershell
cargo test -p argusflow-automation --test native -- --ignored --nocapture --test-threads=1
cargo test -p argusflow-windows --test native -- --ignored --nocapture --test-threads=1
```

固定 OCR 验收使用已准备的官方依赖；不采集用户屏幕：

```powershell
$env:ARGUSFLOW_TEST_DEPS = "$PWD\.deps"
$env:ARGUSFLOW_TEST_RUNTIME = "$PWD\.deps\runtime\cpu"
cargo test -p argusflow-vision --test native official_small_and_medium_cpu -- --ignored --nocapture --test-threads=1

$env:ARGUSFLOW_TEST_RUNTIME = "$PWD\.deps\runtime\cuda"
$env:PATH = "$PWD\.deps\runtime\cuda;" + $env:PATH
cargo test -p argusflow-vision --test native official_cpu_cuda_parity -- --ignored --nocapture --test-threads=1
```

协议替身验证不等同于真实浏览器端到端验收；跨域 iframe、Shadow DOM 和网页焦点行为尚需调用方在实际应用中验收。首期明确限制见 [语言与来源说明](aql.md)。
