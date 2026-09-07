# Rust 开发缓存

`pnpm start` 和 `pnpm rust:run` 会在启动前、正常退出或报错退出时检查本仓库的 `target`。超过 **8 GiB** 时通过 `cargo clean` 清理可重建产物，下一次构建重新编译。这个阈值是启动和退出时的清理预算，不是编译过程中的实时磁盘配额；强制终止进程后会在下次启动时再检查。

```powershell
# 完整开发启动（含 Vite、视觉 worker），自定义缓存预算。
pnpm start -MaxTargetGiB 6

# 直接运行 Rust 桌面程序；前端与视觉 worker 按原 cargo run 方式单独准备。
pnpm rust:run

# 手动执行同一套阈值检查；-WhatIf 只预览。
pnpm rust:cache -MaxTargetGiB 8 -WhatIf
```

清理范围固定为仓库的 `target`，拒绝经过符号链接或目录联接。有 Cargo、rustc 或 ArgusFlow 进程运行时会推迟清理。自定义 `CARGO_TARGET_DIR` 等位置不在此脚本的清理范围内；`.argusflow` 中的工作流、录制和运行记录也不属于构建缓存。

开发配置关闭增量编译，只保留调试行号，测试继承同一配置。直接使用 `cargo run`、`cargo test` 也会使用这个精简配置，但不会执行脚本的 8 GiB 阈值检查。需要完整调试信息时，可临时设置 `CARGO_PROFILE_DEV_DEBUG=2`。

`.cargo/config.toml` 设置 `cache.auto-clean-frequency = "always"`，让每次构建检查 Cargo 全局缓存中的过期项。Cargo 稳定版按最近使用时间清理：可离线重建的缓存闲置 1 个月、需要重新下载的缓存闲置 3 个月后删除；离线或 `--frozen` 模式不清理。这不是 `.cargo` 的容量上限，也不清除已安装工具。参见 [Cargo 缓存配置](https://doc.rust-lang.org/cargo/reference/config.html#cache)。
