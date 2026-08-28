# ArgusFlow Vision Worker

这是 ArgusFlow 的本地 PaddleOCR 3.7.0 worker。worker 只通过 Windows Named Pipe 接收
当前 AppSession 的 ROI 像素，不保存截图，也不把屏幕内容发送到云端。

启动桌面端前，由部署层生成一次随机 session token，并使用相同的 pipe 名称和 token 启动：

```powershell
argusflow-vision-worker `
  --pipe-name '\\.\pipe\argusflow-vision-local' `
  --session-token '<random-session-token>'
```

桌面端读取以下进程环境变量完成连接：

- `ARGUSFLOW_VISION_PIPE_NAME`
- `ARGUSFLOW_VISION_SESSION_TOKEN`

未配置其中任一变量时，视觉 backend 保持 `Unavailable`，不会回退到不受作用域约束的全屏捕获。

当前 P0 传输使用 `AFV2` little-endian 帧头：控制面是 JSON，像素面是同帧 raw BGRA
binary body；控制面和像素体分别限制最大长度。共享内存 ring 保留给后续 P1 性能优化。

worker 生命周期由部署层独占：桌面端超时或管道断开后会把当前 worker 视为 unhealthy，
部署层负责按有界重启策略替换进程。Rust/Tauri 只负责 token 握手、健康状态和请求关联，
不会在应用进程内偷偷复制第二套 worker owner。
