# 原生视频落盘原型

**后续状态：已接入应用。** 当前生命周期、回看和缓存行为以[录制器说明](recorder.md)为准。本页保留原型阶段的调研与测量，下面“尚未接入”等措辞仅描述当时验证范围。

日期：2026-09-13。目标是验证轻量、长时间保存桌面画面的底层路径。当前提供独立命令行原型，尚未替换应用录制入口、接入输入日志或实现按事件解码回看。

## 决策与依据

持续保存压缩视频，图片只作为未来按需解码的派生附件。不能把半小时原生图片保存在内存，也不能把逐帧 PNG 当作默认归档格式。以 2560×1600 为例，一张 BGRA 原图约 15.6 MiB，30 fps 的半小时原始数据约 824 GiB；PNG 大小取决于画面，压缩开销也不能忽略。

采用 DXGI Desktop Duplication → D3D11 VideoProcessor BGRA/NV12 转换 → Media Foundation 硬件 H.264 → 本地 fragmented MP4。生产路径不把完整像素读回 CPU；不加载 OBS、不打包 FFmpeg。FFmpeg 仅用于测试时独立解码核对。

- OBS 的设计说明支持将图形与编码工作分开、用有界队列承受瞬时压力，但其过载处理不能直接当作事件证据的完整性保证。[OBS 后端设计](https://docs.obsproject.com/backend-design)
- Windows 支持把 D3D11 表面包装为媒体缓冲，再通过 Sink Writer 的 DXGI device manager 交给媒体转换。[DXGI surface buffer](https://learn.microsoft.com/en-us/windows/win32/api/mfapi/nf-mfapi-mfcreatedxgisurfacebuffer)、[D3D manager](https://learn.microsoft.com/en-us/windows/win32/medfound/mf-sink-writer-d3d-manager)
- 启用硬件转换只是许可，不证明实际选择了硬件编码器。原型检查实际 transform 的硬件标识；无法确认则报错，不增加软件回退。[硬件转换属性](https://learn.microsoft.com/en-us/windows/win32/medfound/mf-readwrite-enable-hardware-transforms)
- 原生 Media Foundation 提供 fragmented MP4 sink。采用分片是为了录制过程中形成可解码前缀；普通 MP4 的完整结束元数据不适合充当唯一恢复保障。[原生分片 sink](https://learn.microsoft.com/en-us/windows/win32/api/mfidl/nf-mfidl-mfcreatefmpeg4mediasink)。[FFmpeg 对分片的说明](https://ffmpeg.org/ffmpeg-formats.html#Fragmentation)也指出其长文件内存和中断恢复优势，但不据此推断 Windows sink 已具备断电保证。
- 请求单遍 PeakConstrainedVBR，目标 8 Mbps、峰值 16 Mbps、低延迟，约每秒请求关键帧。请求成功不等于任意驱动或画面下具有严格文件大小上限。[码率控制模式](https://learn.microsoft.com/en-us/windows/win32/api/codecapi/ne-codecapi-eavenccommonratecontrolmode)

## 时间、内存与磁盘

原始 QPC 是事实时间，视频 PTS 是可解码时间，两者都写入索引。DXGI `LastPresentTime` 与采集返回时的 QPC 分开保存；`AccumulatedFrames` 只表示累计桌面更新，不能直接认定为丢失业务事件。[DXGI 帧信息](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_frame_info)

首帧作为 PTS=0 的基线，保留原始呈现时间。后续采用整数换算，按绝对时间边界舍入到 1 ms，再计算帧间隔，避免封装逐段量化产生累积漂移。原始 QPC 不舍入；媒体边界通常最多偏移 0.5 ms，首帧基线另行标记。静态画面每秒复用缓存纹理推动编码落盘，标为 repeated，不能冒充新的桌面呈现。

8 张 NV12 纹理在采集、队列、待补时长帧和编码器之间转移，另有 1 张静态缓存。使用 tracked sample 的最后引用释放回调归还纹理，不能在 WriteSample 返回时提前复用。2560×1600 的应用侧纹理预算为 52.73 MiB，超过 256 MiB 的配置拒绝启动；驱动和编码器内部资源另计。持续两秒纹理池耗尽则停止并记录缺口，不能无限积压。

每次会话创建新目录，不覆盖旧文件：

| 文件 | 含义 |
| --- | --- |
| screen.mp4 | 持续写入的 H.264 分片视频 |
| session.json | 来源、尺寸、QPC 时钟、请求码率与媒体时间粒度 |
| frames.jsonl | 逐帧序号、呈现/采集 QPC、PTS、时长、重复标记；流式写入 |
| capture.jsonl | 纹理池缺口和采集结束原因 |
| complete.json | Finalize、索引同步及视频 sync_all 成功后才创建 |

`frames.jsonl` 的 submitted 表示编码器已接受样本，不表示该帧已物理持久化。进程异常退出时，解码器确认的有效视频前缀才可作为恢复范围，索引尾部可能领先视频。本原型尚无启动恢复扫描和中途耐久水位；不能把分片写入操作系统缓存描述为断电不丢。

平均码率 × 秒数 ÷ 8 是容量估算：

| 实际平均码率 | 10 分钟 | 30 分钟 |
| --- | --- | --- |
| 4 Mbps | 0.3 GB | 0.9 GB |
| 8 Mbps | 0.6 GB | 1.8 GB |
| 16 Mbps | 1.2 GB | 3.6 GB |

表中是十进制容量，未计音频、容器和索引；本原型不录音。VBR 的目标码率不是实际平均码率。每秒检查整个会话目录配额和卷剩余空间，低于 256 MiB 或达到配置的软配额就停止，保留已有内容；不自动删除历史。软配额可能超过一个检查周期内的写入量和编码尾部，不能替代文件系统硬限额。命令行示例默认配额 4 GiB。

## 已测量结果

本机 Windows、NVIDIA H.264 Encoder MFT，debug 构建，2560×1600、最大 30 fps、目标 8 Mbps。真实桌面仅被动录制，没有自动操作浏览器或 Notepad。

| 验证 | 实际结果 |
| --- | --- |
| 10 分钟墙钟实录 | 8,650 帧；视频 224,337,584 字节（约 224 MB）；纹理池耗尽 0 次 |
| 该实录进程工作集 | 排除前 30 秒后 73.12–77.75 MiB，按秒采样 |
| 进程 PrivateUsage | 第 30 秒约 151.07 MiB，结束约 154.91 MiB；不包括显存 |
| 进程 CPU | 约 21.08 CPU 秒 / 600.07 墙钟秒，即一个逻辑核的 3.51%；不是整机百分比 |
| 修正媒体量化后的短实录 | 16 帧全部解码，逐帧 PTS 与落盘索引偏差 0 |
| 第 8 秒强制结束进程 | 可解码前缀 194 帧，逐帧 PTS 与索引偏差 0；没有 complete 标记 |
| 30 分钟合成媒体时间轴 | GPU 红/绿/蓝三帧各保持 600 秒，解码颜色及 PTS 验证通过 |

10 分钟资源实测运行于媒体量化修正前，证明该版本的持续写盘和资源表现，不用它宣称修正后 10 分钟逐帧 PTS 已验证。30 分钟合成测试验证时间轴，不等于 30 分钟真实负载测试。没有测 GPU 利用率或驱动显存；不能把应用纹理预算当作总显存。内容不同会显著改变视频大小，不能据本次静态/办公混合画面推断所有 30 分钟录制只占约 673 MB。

Rust 格式、工作区 all-targets Clippy（warnings-as-errors）和工作区测试通过。新增视频测试单独包含 ignored 原生测试运行：7 通过、0 失败；覆盖大 QPC、30 分钟边界量化、非法时长拒绝、不覆盖文件、配额不删旧数据、样本最后引用释放和真实硬件编码。工作区默认 ignored 的其他原生验收不计为通过。

## 复现

下面命令会在给定新目录中保存当前桌面，目录必须不存在。录制在独立进程运行；探针带超时监督，因为同步驱动和 WriteSample 调用不保证可取消。[WriteSample 的阻塞语义](https://learn.microsoft.com/en-us/windows/win32/api/mfreadwrite/nf-mfreadwrite-imfsinkwriter-writesample)

```powershell
cargo build -p argusflow-windows --example record_video
python tests/argusflow-windows/support/video_probe.py .cache/video-check --seconds 600
python tests/argusflow-windows/support/verify_video.py .cache/video-check

python tests/argusflow-windows/support/video_probe.py .cache/video-kill --seconds 30 --kill-after 8
python tests/argusflow-windows/support/verify_video.py .cache/video-kill --incomplete

cargo test -p argusflow-windows capture::video::tests -- --include-ignored --nocapture
# 用测试输出中的 SYNTHETIC_VIDEO 路径替换下方路径
python tests/argusflow-windows/support/verify_video.py .cache/video-synthetic-ID --colors
```

探针使用 Python 标准库；独立解码验证脚本需要 imageio-ffmpeg，仅为本地测试依赖。长测试时可复制 EXE 并通过探针 `--exe` 指定，避免占用 Cargo 输出导致重新链接失败。

## 接入主录制器前的剩余工作

需要将输入事实与此视频共用时钟，建立输入→呈现区间索引，按需 seek/解码图片并加入有界派生缓存；将独立进程监督接入现有录制生命周期，完成异常前缀扫描、暂停分段、配额交互和导出。不能继续把一批操作关联到同一张 After 图片。

目前只支持一个未旋转 SDR 输出；不合成 DXGI 独立硬件指针，不录音，不自动处理输出热插拔。需要进一步验证 Notepad 操作关联、文本/OCR 清晰度、30 分钟真实负载、其他显卡、慢盘及满盘错误。此原型验证了底层路径，没有把现有 UI 的错误截图问题标记为已修复。
