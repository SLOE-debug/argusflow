# 原生 OCR 依赖与故障排查

当前可复现原生包面向 Windows x64。编译需要 Rust MSVC 工具链和 Visual C++ Build Tools；运行原生 DLL 需要相应 MSVC 运行库。日常 `cargo build/test` 无模型下载步骤，`ort` 使用 `load-dynamic`，不是下载二进制的构建模式。

## 固定版本和下载

| 依赖 | 固定版本 |
| --- | --- |
| Rust ort | `=2.0.0-rc.12`，API 24 |
| ONNX Runtime CPU/GPU | 1.24.2 |
| CUDA runtime | 12.8.90 |
| cuBLAS / cuFFT | 12.8.4.1 / 11.3.3.83 |
| cuDNN | 9.8.0.87，CUDA 12 包 |

GPU 使用 CUDA 12.x/cuDNN 9.x 组合；匹配依据为 [ONNX Runtime 官方 CUDA 依赖文档](https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html)。未要求升级驱动，本机驱动 576.52 已完成真实验证。

```powershell
# 根据需要选择 cpu、cuda 或 all；模型两档均准备。
powershell -ExecutionPolicy Bypass -File scripts/prepare-native-deps.ps1 -Device all
```

脚本只下载到仓库 `.deps`，每项文件/压缩包校验 `scripts/native-deps.lock.json` 中的 SHA-256。模型来自 PaddlePaddle 官方 Hugging Face 仓库，URL 固定 revision，不跟随 main。ORT 来自微软 release，CUDA/cuDNN 来自 NVIDIA redistributable。模型和 DLL 不进入 Git。

模型修订：

| 模型 | revision |
| --- | --- |
| Small det | `28fe5895c24fd108c19eb3e8479f4ab385fbfc62` |
| Small rec | `b8f84f0b80c529de40b4fbb3544b84fa7233a513` |
| Medium det | `61323801669c338b7891481ec7bac61ce31b576a` |
| Medium rec | `50c7eacafc52fa7bcf4194e8cd08e46f8558504b` |

配置和词典直接使用同 revision 的 `inference.yml`。推理前复核模型和 YAML SHA-256，损坏或误换模型直接失败。算法参数参考 [PaddleX 检测处理](https://github.com/PaddlePaddle/PaddleX/blob/develop/paddlex/inference/models/text_detection/processors.py) 和 [识别处理](https://github.com/PaddlePaddle/PaddleX/blob/develop/paddlex/inference/models/text_recognition/processors.py)。所有运行、下载与测试不需要 Python，不添加 C++ 包装库。

`.deps/models/{small,medium}/{det,rec}` 保存 ONNX 和 YAML；`.deps/runtime/{cpu,cuda}` 保存 DLL；`.deps/archives`、`.deps/unpacked` 为已校验下载和解包缓存。脚本保留原生包内 LICENSE/NOTICE。再分发模型与 NVIDIA 原生依赖时请保留上游许可并遵循相应再分发条款。

## 运行

```powershell
# CPU 单独运行
cargo run -p argusflow-vision --example recognize -- .deps tests/argusflow-vision/fixtures/bilingual.png cpu small

# CUDA DLL 依赖发现只修改当前 PowerShell 进程环境，不修改系统 PATH。
$env:PATH=(Resolve-Path .deps/runtime/cuda).Path+';'+$env:PATH
cargo run -p argusflow-vision --example recognize -- .deps tests/argusflow-vision/fixtures/bilingual.png cuda medium
```

ORT 是进程级动态库，不能在同一进程先加载 CPU DLL 再切换 GPU DLL。需要同进程比较或同时使用 CPU/CUDA 时，所有 OcrConfig 的 `runtime_directory` 显式指向同一个 `runtime/cuda`，然后分别选择 Device::Cpu 和 Device::Cuda；这不是自动设备回退。

```powershell
# 模型直接加载/推理探针，不做图像后处理
$env:PATH=(Resolve-Path .deps/runtime/cuda).Path+';'+$env:PATH
cargo run -p argusflow-vision --example model_probe -- .deps cuda

# 完整固定图片验收
$env:ARGUSFLOW_TEST_DEPS=(Resolve-Path .deps).Path
$env:ARGUSFLOW_TEST_RUNTIME=(Resolve-Path .deps/runtime/cuda).Path
cargo test -p argusflow-vision --test native -- --ignored --test-threads=1 --nocapture
```

CPU-only 机器不设置 `ARGUSFLOW_TEST_RUNTIME`，只运行 `official_small_and_medium_cpu`：

```powershell
$env:ARGUSFLOW_TEST_DEPS=(Resolve-Path .deps).Path
cargo test -p argusflow-vision --test native official_small_and_medium_cpu -- --ignored
```

## 故障定位

| 现象 | 处理 |
| --- | --- |
| 依赖目录/模型/DLL 缺失 | 在使用前显式运行准备脚本；核对传入 dependencies 绝对路径 |
| SHA-256 不匹配 | 不跳过校验，重新运行脚本从锁定 URL 下载 |
| CUDA EP 初始化失败 | 查看错误 source；检查 runtime/cuda 在当前进程 PATH，DLL 齐全，device_id 有效、显存可用 |
| Windows DLL error 126 | 检查传递依赖和 MSVC 运行库；这不一定意味着 onnxruntime.dll 本身不存在 |
| 已加载另一份 ORT | 统一 runtime_directory 后启动一个新进程 |
| Busy | 复用引擎/服务，等待现有请求或清理完成，不创建替代线程 |
| Unresponsive | 原生调用尚未退出；shutdown 有界返回而不是伪报回收。持续卡死时由宿主决定结束进程 |
| ResourceLimit | 图片/候选/查询预算超限；缩小输入或显式调整允许范围内的配置 |
| CDP/UIA 不支持操作 | 检查真实控件 Pattern 或协议能力；不会隐式换成其他操作 |
| 结果未确认 | 检查目标当前状态，不自动重放点击、输入、导航或脚本 |

`prepare-native-deps.ps1` 需要能访问官方域名，网络失败最多重试三次；识别过程中没有任何网络下载。可以把已准备的 `.deps` 复制到离线机器并保持目录结构。
