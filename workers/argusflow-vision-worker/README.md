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

当前 P0 传输使用 little-endian 长度前缀的 framed JSON 和 inline BGRA ROI；共享内存 ring
保留给后续 P1 性能优化。
