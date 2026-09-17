# 录制数据格式 v2

权威类型在 `argusflow-input-contracts` 和 `argusflow-recorder/model`；没有旧格式迁移或兼容分支。导出的 README 同时包含独立阅读说明。

应用当前使用独立视频作为画面来源，输入日志框架仍为v2。下面的Visual/Ocr字段描述已有记录类型，当前录制不再逐事件写入PNG Visual。回看使用Interaction QPC查询video片段，按需生成可重建图片；不把派生缓存当作原始日志记录。

## 文件与完整性

| 文件 | 内容 |
| --- | --- |
| `session.json` | format=2、UUID、created_ms、qpc_origin、qpc_frequency、capture_session、本次策略 |
| `events.afr` | 权威有序追加记录 |
| `attachments/<BLAKE3>.png` | 独立附件，只保存相对路径、哈希、字节数和尺寸 |
| `video/<六位片段号>/screen.mp4` | 持续落盘的原生H.264分片视频 |
| `video/<片段号>/session.json` | 片段qpc_origin/qpc_frequency、width/height、source.origin/dpi |
| `video/<片段号>/frames.jsonl` | frame.sequence、pts_100ns、presented_qpc、acquired_qpc、repeated、duration_100ns、status=submitted |
| `video/<片段号>/capture.jsonl` | 采集缺口与结束原因 |
| `video/<片段号>/ready.json / complete.json` | 首帧就绪 / Finalize及同步完成 |
| `review-cache/<BLAKE3>.png` | 回看派生缓存，32张/256MiB，允许淘汰；不导出 |
| `review-frames.json` | 样本到内容寻址PNG的派生映射，最多32项；不导出，不是原始证据 |
| `review-analysis.json` | 像素分析派生结果，最多128项，包含源身份、区间、序号、变化区域和状态；不导出 |
| `records.jsonl` | 导出时逐条输出的Record JSON，可流式消费 |
| `timeline.md` | 人类可读操作摘要与原始引用 |
| `README.md` | 格式、时间、坐标、证据限制 |
| `FORMAT.md` | 本格式说明，随导出包独立提供 |
| `recovery-tail-<offset>.bin`、`recovery.json` | 非活动录制恢复时隔离的损坏尾部及恢复边界（仅有故障时） |

每个二进制帧为 `AFR1`（4字节）+ JSON长度（u32小端，1..1048576）+ 原JSON字节的BLAKE3（32字节）+ UTF-8 JSON。`Record={id,written_qpc,data}`，id从1连续递增。data使用Serde外部标签，例如 `{"Raw":{...}}`，不会修改既有记录来填写后续结果。

Written通知只表示write_all完成；Synced水位只在sync成功后更新。持久记录`Durability.through`证明该序号及以前的前缀成功同步。watermark记录本身再次同步，但其through仍是前一次同步边界，不能据此推断断电时整条末尾必然存在。写失败会锁定写者，禁止后续推进水位。

重新打开非活动会话会顺序校验魔数、长度、哈希、JSON和连续序号。遇到坏尾部先独立同步保存尾部，再截断权威文件到完整前缀，追加Interrupted；完整日志缺少终止状态也追加Interrupted。不会猜测、跳过坏帧后拼接后续内容，也不能恢复未提交队列。恢复追加记录的written_qpc是重新打开进程的处理时刻，可能来自另一次系统启动，**不得用于推导原会话输入顺序**；原始事件的时间和内容不变。

附件写入临时文件并sync后原子发布，确认后才写成功引用。读取同时核对约定相对路径、字节数和哈希。导出到临时目录，仅复制有有效记录引用的附件，生成JSONL和摘要后重命名为目标新目录；不覆盖原数据。复制后的目录可独立读取，不需要开发机路径或运行时句柄。

## 记录与引用

| 标签 | 关键字段／语义 |
| --- | --- |
| Raw | sequence、qpc、system_time、point、window、foreground_handle、origin、flags、kind；实际收到的事实 |
| Interaction | raw[]、kind、from_qpc、through_qpc、window、basis、related；保留识别依据，文本可能未确认 |
| Structure | raw[]、source、relation、from_qpc/through_qpc、identity、properties、ancestors、bounds、truncated、stale、sensitive、target_confirmed、scope |
| Visual | raw[]、relation、version、presented_ns/acquired_ns/frozen_ns、from_ns/through_ns、region、screen_origin、dpi、status、decision、image |
| Ocr | raw[]、image_hash、version、screen_origin、image_size、screen_size、blocks[{text,confidence,polygon}] |
| Association | records[]包含已提交视觉记录及其观察区间内的输入；relation说明时间共现与索引截断 |
| Attempt | raw[]、stage、outcome、reason；空raw表示会话级来源生命周期尝试，其余引用已提交输入 |
| State | phase、reason；Recording、Paused、Stopping、Stopped、Faulted、Interrupted |
| Durability | through；成功同步的连续前缀 |

Interaction类别为Click、DoubleClick、Drag、Scroll、Chord、SystemKey、TextUnconfirmed、ImeUnconfirmed、PasteUnconfirmed、DeleteUnconfirmed、TextObserved、WindowSwitch、Unresolved。SystemKey表示左右Win键释放事实，不推断开始菜单是否打开。当前观察到的实际文本保存在Structure属性，归一化不会拿键码生成TextObserved。DoubleClick.related是第一次点击的原始释放记录ID；第一条Click保持不变，时间线隐藏它，只显示组合操作。每个输入仍只有系统监听归一化链，CDP事件作为附加证据，不另生一份操作。

Attempt.stage为Input、Journal、Uia、Cdp、Anchor、Visual、Ocr、Derivation；outcome为Complete、NotConfigured、Unavailable、Unresolved、TimedOut、BudgetExceeded、Stale、Cancelled、Failed。NotConfigured表示用户未启用的可选能力，保留原因但不计入证据缺失；已配置来源加载或执行失败仍计入。Derivation Complete只表示本次收集调度已结束；各来源失败仍保留，不能作为“证据完整”使用。日志故障、结构不足、视觉缺口和同步水位是独立维度。

Structure.properties是有限JSON属性树（文字、数字、布尔、空值、列表、对象）。UIA含role/name/AutomationId/class/enabled/focused/password及可读取实际value。CDP含page_now_ms、document_focused、page_lost、page_events、focused_target；事件包含sequence、kind、time、trusted、input_type、target与qpc_interval。目标有tag/role/name/id/input_type/value/password/bounds/context/truncated；bounds是视口CSS像素，不能当屏幕物理坐标。

Visual.status为Anchored（输入前最近缓存帧）或Observed（输入结束后的采样）。录制不计算真实变化次数、净变化区域或像素稳定性，相关旧字段已删除。decision为带原因的StructuredSufficient、VisualRequired或SensitiveOmitted；当前全屏适配使用后两种。image为空不代表无变化。

采样或事件帧缺失用独立Attempt记录；可用的补充帧照常保存。`relation=Input`：每个raw引用分别在该事件到达时固定了这一版本；相同版本可以共享多个raw引用，from_ns/through_ns是首末引用的事件时间，精确单事件QPC在Raw中。`relation=Response`：逐事件短暂等待得到的新呈现版本，raw只包含相应按下/松键或窗口事件，不自动扩为整组输入；from_ns为输入起点，through_ns为受后续输入上界约束的观察水位。Before仍表示前置观察，不重解释其关联。编码繁忙时保留事件和补充版本，在有限预算内合并引用。图片不证明应用已完成请求。

## 时间、坐标与有效性

视频的`pts_100ns`相对于所属片段的QPC原点，单位100ns，绝对边界量化到1ms；首帧映射到0，其原始QPC另存。换算QPC为`segment_origin + pts_100ns*frequency/10_000_000`。输入时间来自Raw/Interaction，暂停空洞没有画面，不跨空洞寻找“最近截图”。静态repeated帧的媒体时间前进，原始呈现QPC保持不变。submitted不等同已落盘，回看必须解码核对真实PTS。没有complete的片段只能使用可解码前缀，不修改原索引伪造完成。

QPC在同次系统启动内跨父子进程共享。输入相对采样纳秒按`(qpc-qpc_origin)*1e9/qpc_frequency`换算；created_ms为UTC Unix毫秒，仅供列表展示。system_time是Windows输入结构的毫秒tick，不与QPC或UTC直接混用。written_qpc为写处理开始时刻，不是输入发生时间或完成确认时间。

Structure的主机请求区间为[from_qpc,through_qpc]。页面事件e的QPC区间为`[from,through]-(page_now-e.time)*frequency/1000`；该区间有通信和调度宽度，保留原页面时间。target/frame/document_epoch限定文档身份；After结构是后观察，嵌套页面事件按各自qpc_interval判断，不凭所属记录推断发生在系统输入之后。

Visual.version由帧源session（十进制字符串）、source、generation、revision组成，revision随接受的桌面帧递增，不是像素变化次数。from_ns/through_ns表示观察范围，presented_ns为真实呈现时间（未知则null），acquired_ns、frozen_ns分别为该帧采集与冻结时间。全部纳秒统一换算到session.json记录时钟域；暂停后帧源重建拥有新session和origin，不能直接拼接新源的相对时间。静态屏幕可以复用旧帧，但through_ns必须是输入之后真实检查过的健康水位；不改写该帧的呈现时间。

region为完整显示器物理范围`[0,0,width,height]`，screen_origin为显示器左上角屏幕物理位置，允许负数。附件等比缩到1920×1080范围内，不放大小屏幕；16:10屏幕输出1728×1080。OCR四边形保持附件坐标，物理坐标为`screen_origin + polygon * screen_size / image_size`（各轴分别计算）。DPI只记录来源信息，不再额外乘一次缩放。每屏独立取图，不承诺多屏原子同步。

HWND/PID/epoch、UIA属性和CDP身份都是会话内线索，不能保证未来重连定位或回放。程序应流式读取并按ID关联，保留失败和原始事实；时间共现、像素稳定、识别到文字均不证明业务成功或未来AI必然理解。
