# 屏幕变化算法与录制证据

## 检索结论

Context7 `/websites/webrtc` 没有 native desktop differ 文档，进一步核对了上游源码与 Microsoft 文档：

- [DXGI Desktop Duplication](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)：提供 dirty/move rectangles。dirty 可合并并包含未变化像素，重建时先处理 move，再处理 dirty。
- [DXGI frame metadata](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_frame_info)：LastPresentTime 区分桌面呈现和仅鼠标更新；AccumulatedFrames 表示积累帧，不能声称采到了所有显示刷新。
- [WebRTC block differ](https://webrtc.googlesource.com/src/+/refs/heads/main/modules/desktop_capture/differ_block.cc) 和 [SSE2 核](https://webrtc.googlesource.com/src/+/refs/heads/main/modules/desktop_capture/differ_vector_sse2.cc)：分块、SIMD、发现差异提前退出。当前 Rust 核独立实现 XOR/OR 精确相等检测，不依赖 WebRTC。
- [xxHash 官方基准](https://xxhash.com/) 是指定 CPU 和编译配置上的哈希吞吐量，不能直接换算为低端机器录屏性能。哈希不能省去读取候选像素，碰撞也不适合成为唯一相等判据。

## 当前实现

独立观察线程最多约 30Hz，所有事件共享一次采样与一次 diff。无输入时也采样，隐私配置禁止截图时完全关闭像素读取。DXGI 没有新桌面呈现时返回无更新，跳过整帧 CPU 读回和比较。新画面使用 32×32 分块，x86-64 基线 SSE2，局部块找到一行差异便提前退出，无灰度转换、缩图、哈希或差异图片分配。

近期帧使用只读 Arc，共享静态像素；历史最多 16 个时间点、按 128MiB 限制帧数。PNG 队列额外有界。点击根据实际输入时间选择其之前、最多 200ms 的已观察画面；目标与结果分别保存，不再由输入线程访问截图后端。画面稳定只表示安静区间，不能证明业务加载完成。

变化检测现在一次扫描同时输出变化块坐标，各事件只累计与其相关窗口相交的变化块并据此判断安静区间。结果 PNG 裁为相关变化的包围盒并加 24px 语境边距；无变化范围时保存窗口区域。点击目标从输入前帧裁取窗口区域，缺少窗口范围时使用点击周围局部区域。所有裁图保留真实屏幕原点。

当前仍有明确边界：有新呈现时读回完整虚拟桌面；dirty/move rectangles 尚未接入区域级 GPU 读回。此次新增的是证据区域裁剪，不等同于采集带宽优化。采样低于刷新率，不能保证保留只出现一个刷新周期的菜单。不能用“稳定”或本机微基准保证低端机器端到端性能。

## 可复现微基准

运行 `rustc -O --edition 2024 scripts/bench-screen-diff.rs -o target/bench-screen-diff.exe` 后执行产物。纯内存合成画面，不读取用户桌面，不含采集、GPU 读回、PNG 与磁盘。每组 10 次预热、100 次计时；默认目标 ISA，不使用 target-cpu=native。

2026-09-07 本机一轮测量，单位 ms：

| 画面 | 系统切片比较中位数 | SSE2 中位数 | SSE2 P95 |
|---|---:|---:|---:|
| 1080p 静态 | 0.300 | 0.271 | 0.374 |
| 1080p 单像素变化 | 0.294 | 0.267 | 0.394 |
| 1080p 小区域变化 | 0.290 | 0.264 | 0.367 |
| 4K 静态 | 1.770 | 1.791 | 2.133 |
| 4K 单像素变化 | 1.756 | 1.750 | 2.153 |
| 4K 小区域变化 | 1.785 | 1.757 | 2.182 |

全屏均匀变化在每个块首行立即命中，1080p/4K SSE2 中位数分别约 0.005/0.030ms；这是提前退出的特殊合成场景，不代表一般动画开销。没有低端实体机测量，以上数据不可标为低端机器承诺。
