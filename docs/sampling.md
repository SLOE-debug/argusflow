# 共享采样基础设施与 Paddle 接入

本轮实现 Windows 可见 SDR 桌面的持续状态追踪、版本历史、变化订阅、稳定区域读取与 Rust Paddle OCR 复用。输入监听、交互归一化、独立日志进程和持久化属于后续录制器，本轮只规定其接入契约。实现参考 experimental 的经验重新组织，没有迁入旧采集协议、阈值过滤或兼容分支。

## 依赖和所有权

```text
应用装配 ── CaptureService ── DesktopBackend ← DxgiBackend
               ↑ RegionSource                  每适配器原生线程
            SampledOcr ── OcrEngine             D3D11 / DXGI

Capture / Windows / Vision → capture-contracts → core
```

`argusflow-capture-contracts` 只依赖标准库、core 和错误库，不依赖 Tokio、Windows、图像编码器或 OCR。契约包含来源、时钟、版本、区域、只读像素、字节租约、`DesktopBackend`、`SnapshotPixels` 和 `RegionSource`。服务协调和纯区域算法属于 `argusflow-capture`，全部原生资源属于 Windows 后端。

应用显式启动一个 `DxgiBackend`、一个消费其事件的 `CaptureService`，再克隆服务交给消费者。重复建立原生后台或两个服务抢同一个事件桥都会返回 `Busy`。消费者 Drop 不关闭其他消费者；应用在 Tokio runtime 结束前调用 `shutdown`。关闭具有总时限；原生线程未实际退出时继续持有实例占用，禁止通过重复启动堆积线程。后台异常不会产生健康水位。

## 公开接口

| 入口 | 行为 |
| --- | --- |
| `sources()` | 输出身份、物理范围、旋转、有效 DPI、代际、健康状态及最近失败 |
| `subscribe_changes()` | 每个订阅独立游标；只读元数据，不触发像素读取 |
| `ChangeSubscription::poll(limit)` | 返回真实顺序记录，落后时另附 `lost_sequences` 和缺口，不伪造日志序号 |
| `clock()` / `now()` | 返回共享 QPC 时钟域和相对单调纳秒 |
| `pin_at(source, at, options)` | 固定 at 之前可证明存在的版本；不确定时拒绝锚定 |
| `observe(anchor, scope, policy, options)` | 稳定终点、最终净变化、过程摘要和扩展区域图像 |
| `read_regions(anchor, regions, options)` | 读取固定版本；过期、重建和跨会话均不能替换成当前截图 |
| `RegionSource::sample(request, operation)` | 完整区域稳定采样，精确比较前次内容令牌后决定是否读回 |
| `stats()` / `restart()` / `shutdown()` | 工作量和资源统计、显式新一轮恢复、有界关闭 |

`OperationOptions` 是从调用开始计时的总时限。底层读取及 `RegionSource` 接受同一 `Operation`；取消会阻止后续步骤，已提交 GPU 工作在完成或设备重建前保留资源。丢弃 Future 会协作取消。并发额度满立即 `Busy`，图像、区域和内存超限返回明确错误，不降分辨率。

## 像素、版本和时间

`Version = (session, source, generation, revision)`。正常初始化及重建基线 revision 为零，只有 RGB 确实发生变化才增加 revision。事件桥发生容量缺口后可以重新交付已存在版本作为恢复基线；这不是一次新像素变化。来源重新建立后递增 generation。来源停止或真正重建时撤销旧版本；撤销也释放失效句柄背后的闲置 GPU 纹理，避免迟到 OCR 缓存占据恢复预算。

QPC 原始 origin/frequency 随 session 导出，呈现、采集、冻结和已处理水位分开保存。输入采集进程应使用原始 QPC 并按同一 origin/frequency 转换，不能把墙钟、毫秒 tick 或别的采样会话混进来。跨线程相差约一个 QPC tick 的边界按不确定处理；`pin_at` 不选择此边界作为确定的交互前版本。[微软 QPC 文档](https://learn.microsoft.com/en-us/windows/win32/sysinfo/acquiring-high-resolution-time-stamps)

坐标始终为物理像素。`PixelRect` 位于旋转后的来源本地空间；`ScreenRect` 允许负原点，`project` 转换到虚拟桌面。DXGI 原始纹理坐标仅在 Windows 后端内部使用。每个来源独立发布版本与水位；跨屏调用方组合各屏结果并保留各自版本/时间范围，不宣称多屏原子同步。

`PixelImage` 克隆共享只读缓冲，不复制像素；最终引用释放后归还 CPU 字节额度。明确区分不透明 BGRX 与带透明度 BGRA，X 通道不参与差分，也不参与透明合成。

## GPU 追踪与按需读回

每个硬件适配器一个工作线程串行拥有 immediate context。同输出只有一个 duplication。输出公平轮转，`AcquireNextFrame` 单次等待不超过 8 ms，已有 GPU 或读取工作时采用非阻塞轮询，循环让出线程，避免紧密等待。独立光标更新不产生桌面 RGB 修订。[AcquireNextFrame](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe)

dirty rectangles 与 move destinations 只作候选；移动目标直接从 DXGI 当前完整源纹理取最终像素，不在同一纹理内重放覆盖。候选经 32×32 块位图去重，在 GPU 上逐像素比较有效 RGB 通道，输出每块真实变化计数、比较计数和最小变化边界。无缩略图、感知哈希、相似度或面积阈值。块内矩形允许覆盖未变像素，但不能漏掉真实变化。DXGI 本身可能合并脏区，因此不能拿驱动脏区直接当真实差分。[Desktop Duplication](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)

实时 GPU 基线持续更新；历史使用不可变块索引，64 个条目一组写时复制。候选块先冻结到有限 atlas，差分完成后只保留真实变化块，稀疏变化再压紧 atlas，防止单个变化块长期固定整批候选。相邻版本共享未变纹理。发布必须等待差分和最终冻结复制完成；后续快照不能提前进入版本索引。服务额外核对快照身份、修订顺序和时间，错误后端事件导致明确 `Protocol` 失败。

只有观察、固定版本读取和 OCR 采样请求会读回图像。区域可逐批提交，staging 资源可复用；`Map(DO_NOT_WAIT)` 和 GPU event query 检查完成，不在每个区域紧密 Flush。旋转仅作用于已请求的 CPU 区域。追踪路径只读回小型差分元数据，不存在图像编码路径。[CopySubresourceRegion](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-copysubresourceregion)

## 历史、稳定和缺口

普通历史保留约一秒及窗口起点前必要的最近基线，另设版本数量上限。锚点默认五秒内可启动请求；已经接受的观察固定起点直到完成，不受普通历史淘汰和普通租约到期影响。消费者持有有效内容令牌也持续占用 GPU 资源，应及时释放或清空 OCR 缓存。

默认交互观察从输入锚点至少经过 300 ms，目标范围连续 150 ms 无真实变化，稳定等待最多 2 s。静态画面不要求不断重复帧，但要求采集健康水位达到请求开始时刻、已排空此前提交差分，且没有水位之后的未确认快照。采集卡住和 GPU 在途不能被计时器当作静止。稳定只表示这段观察窗口内的像素状态，应用业务完成仍需自己的语义信号。

观察保存起终点净差异及期间真实变化范围、次数和缺口。`A→B→A` 返回 `StableUnchanged`，过程变化数仍为二，不返回多余图像。多个输入可以分别锚定并得到相同终点版本，后续记录器可以共享同一个视觉结果。结果区分 `StableChanged`、`StableUnchanged`、`TimedOutUnstable`、`HistoryGap`、`SourceUnavailable`、取消或操作错误；超时不会变成稳定成功。

精确变化边界与图像读取区域分开保存。读取区域默认外扩 16 个物理像素，裁边后做精确并集和去重，分批读取。显式忽略区域只影响消费者的稳定和结果，且会从扩展图像区域再次扣除；不改变共享基线。没有自动忽略插入光标、细笔画或动画。大量区域超过算法/请求数量限制时明确报错，不自动退化为整屏。

`AccumulatedFrames > 1` 标记中间呈现不可完整观察；次数不等于真实像素变化次数。输入时间落入合并刷新区间、普通历史不足、事件桥溢出或来源重建时返回缺口。系统提示受保护内容被遮蔽时标记 `Protected` 并进入来源恢复/不可用，不交付遮黑图作为完整证据。[DXGI 帧信息](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_frame_info)

## 预算及恢复

| 项目 | 默认 |
| --- | --- |
| 自有 GPU 资源 | 每适配器 256 MiB，含实时基线、历史、atlas、staging、差分缓冲与完成标记 |
| 交付 CPU 图像 | 服务总计 128 MiB，交付后的克隆仍计费 |
| 变化元数据 | 原生桥 4 MiB + 服务日志 12 MiB = 16 MiB |
| 并发观察 / 区域采样读取 | 8 / 32 |
| 每适配器原生读取等待队列 | 32 |
| 每适配器 GPU 在途 | 最多 3，包含初始化、差分及消费者读取批次 |
| 原生等待 / GPU 批次时限 | 8 ms / 500 ms |
| 一轮恢复总时限 | 10 s，退避 50 ms 起、单次至多 1 s |
| 来源/版本/区域上限 | 会话累计 512 来源；每屏普通历史 1024 版；区域规划 8192 个 |

预算按照自有纹理/缓冲的实际创建尺寸及 CPU Vec 容量计费，不能等同于驱动显示的整个进程显存：DXGI 提供的纹理、D3D 对象内部开销、驱动对齐/驻留、Rust 索引对象和 OCR 模型工作区不包含在图像载荷计数中。索引和来源也有数量上限。自定义元数据配置时应一起分配原生桥与服务日志额度，保持应用总预算。

普通历史先被释放；外部已固定像素继续计费，过高的新增请求被拒绝。GPU 历史无法继续保存时，维护当前桌面仍可前进，产生 `Capacity` 缺口并建立新代际基线。慢订阅者只使自己的游标落后，不阻塞原生追踪。不能承诺在持续高分辨率全屏动画和有限显存下保持完整历史。

设备错误、桌面切换、输出移除、旋转、分辨率和 DPI 变化均撤销旧来源并建立新基线。后台每 500 ms 查询已枚举适配器的输出及输入桌面名称/可访问状态；此查询不切换桌面。恢复耗尽后保持明确不可用，直到拓扑/桌面状态改变或显式 `restart`，不无限创建线程。[OpenInputDesktop](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openinputdesktop)

首版 Windows 10/11、硬件 D3D11、SDR 可见桌面；HDR 颜色空间明确返回不支持。已存在适配器上的多输出、旋转、DPI/显示器变化有实现；运行中新加入全新 GPU 适配器需关闭并重新启动后台以重新枚举适配器。隐藏窗口 WGC、HDR 原生精度、中间画面完整回放、自动动画过滤不在本轮范围。

## Paddle 装配与缓存

`SampledOcr::new(Arc<dyn RegionSource>, existing_engine)` 使用已经加载的 Rust OCR 引擎。调用方指定完整区域，采样先确认安静窗口，再将共享 BGRX 像素交给现有预处理。无 PNG、磁盘临时文件、Python 或模型参数更改。

缓存每实例八个区域，模型/设备/预处理配置由实例持有的不可变引擎绑定。区域键合并在途任务；令牌携带来源、代际、区域和不可变内容版本。即使候选与区域相交，也要精确比较内容，不能仅凭脏区使缓存失效。内容相同返回复用结果，来源观察版本仍更新；迟到结果只写自己的在途槽位，不覆盖较新任务。最后等待者退出会取消工作，异常任务也会释放槽位。`clear_cache` 释放闲置 GPU 内容令牌。

`SampledOcrResult` 提供 `version()`、`bounds()`、`through()`、`reused()`、原始区域 OCR 结果和 `screen_polygon()`。原始 OCR 四边形保留浮点局部精度，屏幕点转换按整数物理像素取整。跨屏文字应由调用方分别指定每屏完整区域；不自动截断文字变化碎片做推理。

可编译的装配示例在 `tests/argusflow-windows/support/sample_ocr.rs`。以下命令只在显式运行时采样；首次不加载模型而只列举来源，第二条将来源 ID 与坐标替换为所需区域：

```powershell
cargo run -p argusflow-windows --example sample_ocr -- .deps
cargo run -p argusflow-windows --example sample_ocr -- .deps <source-id> 0 0 800 600
```

## 后续录制器接入约束（尚未实现）

输入监听与追加日志由独立进程拥有。原始键鼠输入接收后先提交日志，再异步生成规范化交互和视觉请求；视觉服务进程失败不能阻塞输入日志。监听线程与日志写入之间仍需有界队列/背压及明确失败状态，不能宣称尚未提交的队列尾部在崩溃后一定存在。

原始记录不可修改。建议独立追加 `RawInput`、`Interaction`、`VisualObservation`、`Association`、`AttemptResult`、`Gap` 和 `SessionState`。每个记录有自身稳定递增序号；派生记录引用原始序号或区间。多个输入、多个交互可关联同一视觉记录；重试只追加结果，不覆盖原来的失败。视觉缺失不影响已经提交的输入证据。

所有记录携带会话、QPC 时间域和相关来源/代际/修订，输入与视觉的关联仅表达观察到的时间关系，不宣称严格因果关系。记录呈现时间、采集时间、冻结时间与处理水位，不能把异步完成时间当作输入发生时间。

区分“日志进程写入确认水位”和“同步落盘水位”。确认只覆盖该进程已经提交的记录，断电保证只能覆盖成功同步落盘的水位。记录框架建议包含长度、类型/格式版本、稳定序号及校验信息；重启顺序扫描并校验长度/序号/校验，丢弃或隔离未完成尾部，追加恢复状态和明确缺口，不将尾部猜测成完整事件。

大图像数据独立存储，事件日志只含尺寸、格式、内容校验与数据引用。建议先写临时 blob、完成必要同步/原子发布，再追加可引用记录；失败留下的孤儿 blob 可回收。不能在 blob 尚未完整可用时写入虚假的成功引用。

采集重建、历史不足、队列溢出、日志进程退出、编码重试、磁盘失败分别追加明确状态/缺口。原始日志失败时停止宣称可靠录制并向上层报告；不为了保住视觉吞掉原始输入，也不因为输入记录存在就伪造视觉成功。输入监听、落盘协议和恢复扫描需要自己的进程故障/磁盘故障测试，本轮未实现或验证这些保证。
