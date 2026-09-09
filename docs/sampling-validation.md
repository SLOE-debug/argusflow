# 采样验证与性能报告

验证日期：2026-09-10。Windows x64，Rust/Cargo 1.98.0，D3D11 测试适配器为 NVIDIA GeForce RTX 4070 Laptop GPU。当前可见显示器为 2560×1600，单屏。用户要求全部测试自行完成后，已补跑全部 ignored 原生验收，包括独立 Chrome/Edge 及 UIA/真实键鼠；没有保存用户桌面图像。GPU 微基准只使用测试合成像素。

## 覆盖与实测

| 层次 | 验证内容 |
| --- | --- |
| 契约/资源 | 尺寸与 stride 校验、BGRX、克隆图像的字节额度持续占用与最终释放 |
| 区域算法 | 重叠、重复、裁边、外扩、忽略区；100 组确定性区域与 CPU 位图并集对照 |
| 共享服务 | 静止和 A→B→A 不读回图像；过程摘要；多个锚点共享终点；历史淘汰和合并刷新缺口 |
| 时序 | 水位之后快照不能稳定；静止健康水位不能稳定；迟到修订拒绝；已经接受的观察跨普通租约期限 |
| 压力/生命周期 | 元数据游标缺口及独立订阅；观察队列满和取消归还额度；消费者退出不关闭其他消费者；普通历史淘汰不撤销已固定锚点；持续动画超时携带过程摘要 |
| 多来源/拓扑注入 | 不同来源的水位、代际、负坐标与失败相互隔离；拓扑变化中止旧观察并可读取新代际。此项使用确定性替身，不替代多屏实机 |
| 硬件 GPU | 单像素低色差、块边界、重复脏区去重、忽略 X、GPU 冻结历史、区域 stride、移动覆盖与 CPU 像素对照、撤销释放纹理 |
| 原生 DXGI | 启动、无图像请求零像素读回、32×32 区域 4096 字节、内容令牌、重建后旧版本失效 |
| GPU 故障注入 | 批次时限缩到 1 ns，恢复周期 100 ms；最终进入不可用，随后不继续增加代际 |
| Paddle | 官方 Small/Medium、CPU/CUDA，Shared BGRX 与原 PNG 文字及四边形一致、置信度差小于 0.0001；同键合并、内容复用、来源撤销拒绝迟到推理 |

当前工作区共 69 项测试（61 项普通测试、8 项默认 ignored 原生验收），全部已经执行通过。先用 `--include-ignored --test-threads=1` 执行当时完整的 65 项，再补充并通过 4 项采样故障注入测试；无未执行的注册测试。Chrome/Edge 两种浏览器的交互与所有权验收均通过；Release GPU 基准单独串行执行通过。格式检查、Clippy 全部 target 检查和全部示例构建通过，验收专属 Chrome/Edge 进程无残留。

原始日志保存在被 Git 忽略的 `target/validation/`：`all-tests-chrome.log`（65 项全量）、`sampling-faults.log`（含新增 4 项的 16 项采样测试）、`edge.log`、`gpu-release.log`。可控 Provider panic 是测试故意注入，测试最终断言通过；浏览器输出的 libpng 配置警告未导致验收失败。

最新 DXGI 实测统计示例：3 次原生帧获取、1 次真实像素变化、128,032 字节差分元数据、4,096 字节图像读回、1 个主动重建缺口；自有 GPU 载荷峰值 49,472,096 字节、CPU 图像峰值 4,096 字节。桌面实际变化与采集时机有关，这些工作量不是固定性能基准。

真实桌面验收不假定桌面完全静态。它先确认仅追踪时图像读回为零，再显式请求一个小区域。独立的确定性服务测试验证静态画面不会新增真实变化记录；编码次数为零由采样实现没有编码调用保证。Paddle 的共享入口仍会进入现有预处理和模型工作区，这些内存不算共享采样 CPU 图像租约。

## Release 对照测量

同一硬件上两条路径各运行 100 次：1920×1080 合成桌面，每次改变位于 (64,64) 的 32×32 区域。区域路径执行 GPU RGB 精确比较、区域复制及区域 CPU 读回；整屏基线复制/读回 1920×1080 并在 CPU 逐像素定位非零变化。均复用 staging，包含相同更新负载，无图片编码、OCR、DXGI 事件分发或一秒历史维护。未进行自动交互或真实应用负载测量。

以下是本次 Release 实测原始汇总，时间单位为微秒，字节为 bytes：

| 指标 | 区域路径 | 整屏读回基线 |
| --- | ---: | ---: |
| 样本数 | 100 | 100 |
| 线程 CPU 时间总计 | 46,875 | 218,750 |
| 交付延迟 p50 | 3,041.2 | 5,091.2 |
| 交付延迟 p95 | 3,651.3 | 5,605.2 |
| GPU 时间 p50 | 83.1 | 634.9 |
| GPU 时间 p95 | 184.6 | 638.3 |
| 图像读回字节总计 | 409,600 | 829,440,000 |
| 差分元数据读回字节 | 3,200 | 0 |
| 自有 GPU 载荷峰值 | 16,597,088 | 24,883,200 |
| CPU 图像载荷峰值 | 4,096 | 8,294,400 |
| 历史缺口 | 0（合成顺序负载，无事件桥） | 不适用 |

GPU 时间来自 D3D11 timestamp/disjoint query；线程 CPU 时间来自 `GetThreadTimes`，其采样粒度较粗，应看总计，不能用这组数据推断单次精确 CPU 耗时。墙钟延迟包括 1 ms 完成轮询及调度，明显大于 GPU 自身执行时间。内存统计是库追踪的自有资源载荷，非整进程 RSS 或驱动整体显存；测试计时查询、驱动对象内存未纳入资源载荷。

此负载只改变整屏面积的 1/2025，因此区域图像字节减少与面积成正比。不能把此比值宣称为应用加速倍数；区域碎片、动画、显卡调度、历史保留及 Paddle 推理会改变结果。后续应增加真实应用、多屏、长期动画和大量固定租约负载，记录服务端到端 p50/p95/p99、资源峰值和缺口数量。

## 复现

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cargo test -p argusflow-windows --lib exact_gpu_difference_and_snapshot_history -- --ignored --nocapture
cargo test -p argusflow-windows --test capture -- --ignored --nocapture
cargo test --release -p argusflow-windows --lib region_readback_benchmark -- --ignored --nocapture

$env:ARGUSFLOW_TEST_DEPS='D:\Code\rust\argusflow\.deps'
$env:ARGUSFLOW_TEST_RUNTIME='D:\Code\rust\argusflow\.deps\runtime\cuda'
$env:PATH='D:\Code\rust\argusflow\.deps\runtime\cuda;'+$env:PATH
cargo test -p argusflow-vision --test native official_cpu_cuda_parity -- --ignored --nocapture

# 完整测试：设置上述 OCR 运行环境及浏览器路径后执行
$env:ARGUSFLOW_TEST_BROWSER='C:\Program Files\Google\Chrome\Application\chrome.exe'
cargo test --workspace -- --include-ignored --test-threads=1 --nocapture
$env:ARGUSFLOW_TEST_BROWSER='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
cargo test -p argusflow-browser --test native --test ownership -- --ignored --test-threads=1 --nocapture
```

运行 OCR 前按 [原生依赖说明](native-dependencies.md) 准备匹配本机的官方模型与运行库；路径按实际位置替换。GPU/OCR 验收显式 ignored，普通工作区测试成功不代表这些验收已执行。不要并发运行 GPU 微基准与其他重 GPU 测试，否则对照时间受争用影响。

## 未验证及边界

本机只有一个物理显示器，未执行多屏、实际旋转屏、负坐标排列、混合 DPI 和显示器热插拔实机验收。旋转像素/stride、负坐标、来源水位隔离、拓扑代际切换已有纯算法及故障注入覆盖。桌面锁定/解锁、安全桌面、真实设备移除/TDR、受保护内容和 HDR 拒绝路径未实机验证；没有通过锁定用户电脑或卸载驱动人为制造这些条件。已执行显式设备重建与 GPU 超时故障注入，剩余真实场景仍属于验证限制，不作为交给用户的测试任务。

本轮没有录制器持久化代码，因此不包含输入日志崩溃恢复、同步落盘、磁盘满或跨进程故障测试。进程内无法保证恢复 native access violation 或永久卡死；有界 shutdown 会如实报告未退出。适用范围及后续记录器保证见 [采样设计](sampling.md)。
