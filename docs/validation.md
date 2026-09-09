# 验证记录与复现方法

## 已执行

环境：Windows x64，Rust/Cargo 1.98.0，NVIDIA GeForce RTX 4070 Laptop GPU（8188 MiB），驱动 576.52。验证日期 2026-09-09；没有升级驱动，没有使用 Python。用户明确要求代为测试后，已运行真实 UIA/SendInput 验收及独立 Chrome、Edge 的 CDP 验收。

| 验证 | 结果 |
| --- | --- |
| Small det/rec CPU 直接加载推理 | 通过 |
| Medium det/rec CPU 直接加载推理 | 通过 |
| Small det/rec CUDA 直接加载推理 | 通过 |
| Medium det/rec CUDA 直接加载推理 | 通过 |
| Small/Medium 全处理链 CPU 固定图片 | 通过 |
| Small/Medium CPU/CUDA 文字、置信度、坐标比较 | 通过 |
| 本地模拟 CDP WebSocket | 通过：乱序、无响应、断连、畸形消息、session 销毁、迟到响应、队列满、取消、关闭竞态 |
| 可控 UIA Provider/输入边界 | 通过：超时后拒绝新请求、排队取消、迟到结果、关闭竞态、panic、部分 SendInput 注入与按键释放 |
| 窗口身份与进程资源 | 通过：测试专属不可见 HWND 销毁/身份标记移除后失效；非浏览器辅助进程退出和自有临时目录回收 |
| 真实 UIA 控件操作 | 通过：窗口身份、属性读取、唯一查询/歧义、Invoke、SetValue、Toggle、选择、展开/折叠、控件滚动、ScrollIntoView、缺少 Pattern 报错、租约释放后失效 |
| 真实 SendInput | 通过：Focus、Home/Shift+End、Unicode 文本、修饰键释放、鼠标移动、左右点击/双击、双轴滚轮；后台窗口、窗口外坐标和遮挡目标拒绝输入 |
| 真实 Chrome / Edge CDP | 两者均通过：启动/页面列举、新建页面、CSS 查询/属性/文本/歧义、主文档范围、左右点击/双击、Focus、Unicode/Ctrl+A、双轴滚轮、ScrollIntoView、导航/关闭后的句柄失效 |
| 真实 Chrome / Edge 所有权 | 两者均通过：HTTP 与 Browser WebSocket 连接、附加/脱离、外部连接关闭后原浏览器/页面仍可用、自建浏览器 Drop/shutdown/启动超时回收临时目录 |

`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、工作区 32 项普通测试和全部示例构建通过。5 项原生测试定义默认 ignored；UIA 1 项、浏览器 2 项（各在 Chrome/Edge 运行）、OCR 2 项均已显式执行通过。依赖准备脚本 CPU 重复运行校验通过。

真实验收发现并修复两处问题：UIA 导航成功但没有后继元素时，windows-core 返回的空错误曾被当作 Provider 失败；重复 `DOM.getDocument` 曾使浏览器中已查询的 nodeId 失效。现在 UIA 单独识别空导航结果，保留真正的 HRESULT 失败；CDP 在同一文档内缓存根节点，文档替换后重新获取。两项均有回归测试。

后续目录重组验证：源码已按职责分层，测试及资源统一迁至根目录 `tests/`。重新运行 32 项普通测试全部通过，5 项原生验收仍正确注册为 ignored；显式重跑 Small/Medium CPU 完整图片测试通过，验证新的模型配置与测试图片路径。格式检查、Clippy、全部示例构建以及目录迁移脚本重复执行通过。本次目录调整未重跑真实 UIA、浏览器和 CUDA 验收，上表对应此前的实测记录。目录规则见 [项目结构](layout.md)。

OCR 固定图片 `tests/argusflow-vision/fixtures/bilingual.png` 是测试专用合成图，900×260 白底，Microsoft YaHei 字体，两个文本行：`ArgusFlow OCR 123` 和 `中文识别测试 456`。不含真实用户数据。测试使用固定像素文件，不依赖测试机器字体。`tests/argusflow-vision/integration/native.rs` 从该图确定性产生透明背景和旋转 8° 变体，并产生空白/完全透明图。

每档每设备检查编码字节、文件路径、RGBA 像素入口；文本必须与预期完全相等；每个置信度在 [0,1]，四边形在原图范围；旋转图坐标必须体现斜率。CPU/CUDA 固定原图置信度差小于 0.04，角点单轴偏差小于 8 像素。损坏输入和超限图片必须返回错误，空白与完全透明图片必须为空结果。

真实模型测试用 `#[ignore]` 避免普通 CI 隐式依赖原生环境；本机已经显式运行两项，不能把普通 `cargo test` 的 ignored 当成真实验证成功。依赖和运行命令见 [原生依赖说明](native-dependencies.md)。

## 自动执行真实交互验收

在仓库根目录运行以下 PowerShell 命令。测试自行创建控件、执行操作、读取状态并断言，终端逐项输出 `PASS`，失败则返回非零退出码；不用手动点击截图里的控件。UIA 测试会短暂把专属窗口置前台并使用鼠标键盘，执行期间请勿同时操作输入设备。

```powershell
cargo test -p argusflow-windows --test native -- --ignored --nocapture

$env:ARGUSFLOW_TEST_BROWSER='C:\Program Files\Google\Chrome\Application\chrome.exe'
cargo test -p argusflow-browser --test native --test ownership -- --ignored --test-threads=1 --nocapture

$env:ARGUSFLOW_TEST_BROWSER='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
cargo test -p argusflow-browser --test native --test ownership -- --ignored --test-threads=1 --nocapture
```

浏览器路径是本机实际安装位置，可替换为目标机器的可执行文件。测试以 headless 模式启动真实浏览器，使用独立临时配置目录、本地合成页面和随机调试端口。外部连接测试也只连接测试自己启动的浏览器。不要并发运行所有权验收；它通过运行前后的专用临时目录差集确认清理。验收结束后已检查测试窗口和测试专属浏览器进程无残留。

`test_window` 示例仍可用于人工观察；它只是目标控件窗口，不是自动验收入口。完整 UIA 验收使用 `tests/argusflow-windows/support/window.rs` 中额外包含树和滚动列表的专属窗口。

## 尚未覆盖的真实场景

本机只有一个显示器，虚拟桌面为 `(0, 0, 2560, 1600)`。多显示器负坐标和混合 DPI 的真实输入验收仍未执行；负坐标归一化目前有单元测试，不能替代多屏实测。真实浏览器验收使用 headless，未验证日常有界面浏览器的交互外观。普通桌面的 `SendInput` 不保证安全桌面或提升权限窗口操作成功。

原生永久挂死/访问违规不属于进程内方案能够恢复的保证范围。
