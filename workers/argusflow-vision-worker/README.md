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

当前 v5 传输使用 `AFV2` little-endian 帧头承载 JSON 控制面，OCR ROI 像素通过当前
登录会话内的 pagefile-backed Windows 命名共享内存传递。Rust 在收到 worker 响应前
保持 mapping 租约存活，worker 读取后立即复制为紧凑 RGB 数组；正常请求不落临时图片文件。
仅在显式开启模型输入诊断时，响应帧才携带一份无损 PNG binary body。

worker 在一次 OCR 截止时间超时或管道断开后会主动销毁当前模型池并重新监听同一个
Named Pipe；Rust/Tauri 同时以固定间隔重新执行带 token 的 health handshake。这样超时的
同步 Paddle 推理不会把后续请求永久锁在旧进程上，Planner 也能及时看到恢复后的健康状态。
