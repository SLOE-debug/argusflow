# 录制提前停止回归（2026-09-09）

用户录制约 5 秒却只保存 231ms，完整性原因是 `AccumulatedFrames`。
扩大原生测试到 800×600 自有窗口每 50ms 重绘、持续 5 秒后，开发构建复现为仅保存 147699µs、6 帧。

## 原因与修复

- `argusflow-capture` 从已有热路径拆出后漏配开发构建优化；加入与 core、windows、recorder 相同的 `opt-level = 3`。
- 原生获取、冻结像素和 CPU 差分原来在同一线程串行推进。仅优化构建后，三次测试仍有两次在 2.155 秒和 0.611 秒提前停止。
- 独立原生读回线程通过最多 8 批、128MiB 的冻结像素队列交给差分线程，保持顺序；处理中的批次仍计入内存预算。差分不再阻塞原生取帧。
- 队列过载仍报告不完整并重新请求基准，不静默跳帧；最新帧消费者可以继续。关闭时回收两个线程，线程创建失败也会回收原生线程。

## 验证

`cargo test -p argusflow-recorder native_archive_sustained_capture_and_restart --lib -- --ignored --nocapture`

分离线程后，三次 5 秒测试最后帧分别为 5.021921、5.024634、4.990425 秒，全部 `Complete`。PNG 队列峰值最多 2 项、32768000 字节。此测试使用真实 DXGI 和真实 PNG 归档，不安装键鼠 Hook；临时图像在完整性断言之前删除。

这些数据证明该场景的提前停止回归得到改善，不代表任意 GPU、桌面负载或 4K 场景均零漏帧；此测试也不证明逐键 8/8。

## 资料依据

- [DXGI 帧累计定义](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_frame_info)：累计多次桌面更新不能宣称逐帧完整。
- [异步区域复制](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-copysubresourceregion)：提交复制与 CPU 访问应分离。
- [资源复制与流水线停顿](https://learn.microsoft.com/en-us/windows/uwp/graphics-concepts/copying-and-accessing-resource-data)。
- Context7 `/microsoft/windows-rs` 核对接口；具体调用仍以仓库锁定的 windows 0.62.2 编译验证。
