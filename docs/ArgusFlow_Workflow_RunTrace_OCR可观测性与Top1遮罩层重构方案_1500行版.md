# ArgusFlow Workflow Run Trace + OCR 可观测性 + Focus Mask 重构方案（1500 行核心版）

> 仓库：`SLOE-debug/argusflow`
> 审计基线：`main @ ba76590ef7e83cfadd6162a4cd604815e7a7aa60`
> 基线提交：`feat: 重构视觉场景索引与 AQL 空间查询`
> 审计日期：2026-08-29
> 目标：把每次 Workflow 运行升级为可完整复盘的 Run Trace；能查看节点最终输入、WGC 捕获帧、OCR ROI、真正送入 PaddleOCR 的最终模型输入、OCR 结果、VisualScene、AQL 候选集、唯一命中、SendInput 与动作后验证。
> 范围：Runtime / Agent / Vision / PaddleOCR Worker / Windows WGC / Tauri / Studio。
> 核心约束：不新增第二套 Runtime；不破坏 AQL 的 0/1/N 确定性；不让 bbox/坐标成为 Workflow 业务事实；调试功能不得改变执行语义。

---

## 0. 结论先行

这次不应该只增强 `ExecutionLog`，而应该新增一个以 `run_id` 为根的 **Run Trace**。

当前仓库已经具备大量正确底座：
- `ExecutionEvent` 已有 `run_id / workflow_id / sequence / node_id / kind / payload`。
- Runtime 已经能按节点生命周期 emit 事件。
- `EvidenceBundle / EvidenceSink` 已经有结构化失败取证抽象。
- `OcrRequest` 已有 `request_id / frame_id / topology_generation / roi / profile`。
- `OcrResponse` 已有 `raw_text / confidence / polygon / preprocessing / elapsed_ms`。
- `VisualScene` 已经是 bbox/confidence/frame provenance 的事实层。
- Visual Click 已经严格遵守 0/1/N，不会偷偷 highest-score-wins。

真正缺的是：
- 每次运行没有持久化历史。
- 节点“最终解析输入”不可查看。
- OCR 真正输入的图像不可完整追踪。
- OCR 结果、Scene、Query、Materialized Target 之间没有统一可浏览关联。
- Studio 没有历史 Run Inspector。
- WGC 黄色捕获边框被迫承担了一个它根本不应该承担的“可视化识别范围”角色。

本方案建议建立五个正式能力：

1. `RunTraceStore`
2. `RunArtifactStore`
3. `RunTraceContext`
4. `Studio Run Inspector`
5. `Focus Mask`

最关键的 OCR 判断是：

**当前 `last-ocr-input.bmp` 并不等于 PaddleOCR 最终吃到的像素。**

Rust 保存的是裁剪后的 BGRA `OcrRequest.image`。
Python Worker 收到以后还会：
- BGRA -> RGB
- 自适应放大
- CLAHE 局部对比度增强
- 轻量锐化

真正传给：

`pipeline.predict(prepared.pixels)`

的是 `PreparedOcrImage.pixels`。

所以要真正排 OCR bug，必须同时保存：

- `Captured Frame`
- `OCR Source ROI`
- `Exact Model Input`

这三张图不是重复数据，而是在回答三个完全不同的问题。

Top-1 也必须重新定义。

如果 Query 返回多个合法候选，Runtime 仍必须 `AmbiguousTarget`。
UI 可以把排序第一名显示成 `Candidate #1`，但不能称为 `Selected Target`。
只有最终唯一命中才显示：

`唯一命中 / Selected Target`

因此 Focus Mask 是解释层，不是执行规则。

---

## 1. 本次审计参考的 docs 与实现

重点参考：

- `docs/ArgusFlow_微信OCR键鼠与Studio信息架构重构方案_7335750_1500行版.md`
- `docs/ArgusFlow_微信视觉感知_PaddleOCR_v3.7_实施方案.md`
- `docs/ArgusFlow_Selector_Resilience_Failure_Evidence_Design.md`
- `docs/ArgusFlow_视觉场景索引与AQL空间查询重构方案_1500行版.md`

重点代码：

- `crates/argusflow-runtime/src/execution/engine.rs`
- `crates/argusflow-runtime/src/execution/execution_events.rs`
- `src-tauri/src/commands/workflow.rs`
- `src/features/workflow/studio/useWorkflowStudio.ts`
- `src/components/workflow/execution/ExecutionLog.tsx`
- `crates/argusflow-agent/src/evidence.rs`
- `crates/argusflow-agent/src/evidence_sink.rs`
- `crates/argusflow-agent/src/prepared_plan.rs`
- `crates/argusflow-vision/src/diagnostics/ocr_input.rs`
- `crates/argusflow-vision/src/scene_execution.rs`
- `crates/argusflow-vision/src/runtime/scene_refresh.rs`
- `crates/argusflow-vision/src/ocr/result.rs`
- `crates/argusflow-vision/src/worker/protocol.rs`
- `workers/argusflow-vision-worker/src/argusflow_vision_worker/image_preprocessing.py`
- `workers/argusflow-vision-worker/src/argusflow_vision_worker/worker.py`
- `workers/argusflow-vision-worker/src/argusflow_vision_worker/protocol.py`
- `crates/argusflow-windows/src/capture/surface_set.rs`
- `crates/argusflow-windows/src/input/visual_resolver.rs`
- `crates/argusflow-windows/src/input/visual_query_target.rs`

这些实现已经证明：
ArgusFlow 不需要再造第二套视觉 Runtime。
需要的是把已有事实贯通为可复盘的 Run。

---

## 2. 当前痛点的真实根因

### 2.1 Workflow 事件只活在当前会话

`TauriEventSink` 当前只实时 `app.emit()`。

Studio 当前用 React state 持有：

- `events`
- `running`
- `runId`

新一轮运行开始时会清空事件。

结果：
- 没有 Run History。
- Studio 重启后不能复盘。
- WebView reload 后当前细节丢失。
- 不能把一次失败完整发给开发者。

### 2.2 OCR timeout diagnostics 只是“最近一次”

当前诊断使用：

- `last-scene-timeout.json`
- `last-ocr-input.bmp`

它还是环境变量 opt-in。

这套机制适合：
- Vision integration test
- standalone diagnostics

不适合 Studio 的每次运行历史。

### 2.3 当前保存的 OCR 图不是最终推理图

Rust：
`OcrRequest.image`

Worker：
`prepare_ocr_image(rgb, mode)`

Paddle：
`pipeline.predict(prepared.pixels)`

所以：
`OcrRequest.image != 一定等于 Paddle input`

只要发生 resize / CLAHE / sharpen，就已经不同。

### 2.4 Query 失败不能直接归因 OCR

`TargetNotFound` 可能来自：
- OCR 没识别。
- OCR 识别了，但 confidence gate 没通过。
- Scene stale。
- Observation coverage 不完整。
- AQL anchor 没找到。
- AQL 空间关系过滤掉了。
- 当前节点被 Dirty refresh 误处理。
- 当前 scene 根本不是目标窗口。

`AmbiguousTarget` 更说明 OCR 往往已经成功了。

---

## 3. 必须建立的完整因果链

```text
Run
  -> Workflow Snapshot
  -> Run Inputs
  -> Node
  -> Resolved Node Inputs
  -> Prepared Plan
  -> Backend / Candidate
  -> WGC Captured Frame
  -> Stable Frame Gate
  -> DirtyMap
  -> RefreshPlan
  -> OCR Source ROI
  -> Worker Preprocessing
  -> Exact Paddle Model Input
  -> OCR Raw Result
  -> VisualScene Merge
  -> AQL / VisualQuery
  -> Candidate Set
  -> 0 / 1 / N
  -> Materialized Target
  -> Pre-input Revalidation
  -> SendInput
  -> Post-condition Verification
  -> Node Outputs
  -> Run Finalization
```

这条链任何一段断掉，都会让排错依赖猜测。

---

## 4. 设计原则

1. **Run First**：所有诊断事实先归属 `run_id`。
2. **Append Only**：事件采用 JSONL 追加写。
3. **Exact Input**：必须能得到真正送进模型的最终像素。
4. **Provenance First**：run/node/request/frame/scene 必须可串联。
5. **No Semantic Drift**：trace 不改变执行。
6. **Evidence Reuse**：复用现有 Failure Evidence。
7. **Artifact by Reference**：图片不塞事件 JSON。
8. **Human + Machine**：中文摘要 + raw structured trace。
9. **Local Privacy**：截图/OCR text 默认本地。
10. **Crash Tolerant**：异常中断仍能读 Run。
11. **Bounded Cost**：写日志不能拖慢热路径。
12. **Schema Versioned**：所有持久化格式有版本。

---

## 5. 非目标

- 不新增 OCR 专用业务节点。
- 不把 OCR bbox 持久化成 Workflow selector。
- 不让用户手工拼 OCR 坐标点击。
- 不把 WGC frame 全部保存成视频。
- 不默认 OCR 整个桌面。
- 不默认永久保留截图。
- 不让 Python Worker 管理 Run 生命周期。
- 不用 Run Trace 替代 Failure Evidence。
- 不因为做 Top-1 UI 而 highest-score-wins。
- 不优先做 borderless WGC 而推迟真正可观测性。

---

## 6. 新架构总览

```text
WorkflowEngine
    |
    +--> ExecutionEvent ----------------------> Live Studio
    |
    +--> RunTraceContext
            |
            +--> RunTraceWriter -------------> events.jsonl
            +--> RunArtifactStore -----------> images/json/evidence
            |
            +--> PreparedPlan
            +--> VisionRuntime
                    |
                    +--> WGC
                    +--> OCR Worker
                    +--> VisualScene
                    +--> Query
                    +--> MaterializedTarget
                    +--> SendInput
                    +--> Verification

Tauri Run API
    -> list/get/read/delete/pin/export/open

Studio Run Inspector
    -> Run History
    -> Timeline
    -> OCR Viewer
    -> Focus Mask
    -> Raw Trace
    -> Evidence
```

---

## 7. RunTraceContext

建议：

```rust
pub struct RunTraceContext {
    pub run_id: Uuid,
    pub workflow_id: Uuid,
    pub node_id: Option<String>,
    pub expanded_node_id: Option<String>,
    pub span_id: TraceSpanId,
    pub parent_span_id: Option<TraceSpanId>,
}
```

它只传关联身份。
不传图片 bytes。
不传 COM/WinRT handle。
不写回 WorkflowDefinition。

未来多 Run 并发时，显式 context 比全局变量/task-local 更可审计。

---

## 8. Run 目录结构

```text
.argusflow/
└── runs/
    └── 2026-08-29/
        └── <run_id>/
            ├── manifest.json
            ├── summary.json
            ├── workflow/
            │   ├── definition.json
            │   ├── expanded-definition.json
            │   ├── components.json
            │   └── run-inputs.json
            ├── events/
            │   ├── events.jsonl
            │   └── index.json
            ├── nodes/
            │   └── 000004_<node-id>/
            │       ├── summary.json
            │       ├── resolved-inputs.json
            │       └── outputs.json
            ├── vision/
            │   ├── frames/
            │   │   └── <frame-id>/frame.png
            │   └── ocr/
            │       └── <request-id>/
            │           ├── 01-request.json
            │           ├── 02-source-roi.png
            │           ├── 03-model-input.png
            │           ├── 04-result.json
            │           ├── 05-scene.json
            │           ├── 06-query.json
            │           ├── 07-candidates.json
            │           ├── 08-selection.json
            │           ├── 09-overlay.json
            │           └── 10-timings.json
            └── evidence/
                └── <evidence-id>/...
```

P0 可以直接存文件。
P1 再做 sha256 content-addressed blob 去重。

---

## 9. Run Manifest

推荐：

```json
{
  "schema_version": 1,
  "run_id": "...",
  "workflow_id": "...",
  "workflow_name": "发送微信群消息",
  "repository_revision": "ba76590...",
  "app_version": "...",
  "started_at": "...",
  "finished_at": "...",
  "status": "failed",
  "trace_level": "diagnostics",
  "event_count": 84,
  "ocr_request_count": 5,
  "evidence_count": 1,
  "artifact_bytes": 7342281,
  "trace_degraded": false,
  "failure": {
    "node_id": "...",
    "code": "ambiguous_target",
    "trace_sequence": 72
  }
}
```

status：
- starting
- running
- completed
- failed
- aborted
- crashed

---

## 10. RunTraceEvent

推荐 JSONL envelope：

```json
{
  "schema_version": 1,
  "trace_sequence": 118,
  "workflow_sequence": 21,
  "timestamp_utc": "2026-08-29T09:40:13.420Z",
  "monotonic_ns": 284117290331,
  "run_id": "...",
  "workflow_id": "...",
  "node_id": "...",
  "expanded_node_id": null,
  "span_id": "...",
  "parent_span_id": "...",
  "subsystem": "ocr",
  "kind": "ocr_request_prepared",
  "level": "info",
  "status": "ok",
  "message": "已准备 OCR 输入",
  "payload": {},
  "artifacts": []
}
```

保留现有 `workflow_sequence`。
新增全局 `trace_sequence`。
耗时使用 monotonic clock。
用户时间显示 wall clock。

---

## 11. ExecutionEvent 与 RunTraceEvent 的关系

`ExecutionEvent`
= 产品协议。

`RunTraceEvent`
= 诊断协议。

不要强迫普通用户理解：
- DirtyMap
- Worker queue
- preprocessing
- scene merge
- query filter stage

现有中文 ExecutionLog 继续保留。
Run Inspector 展开才显示内部 trace。

---

## 12. “输入”必须拆成两种

### 12.1 节点输入

最终解析后的：
- literal
- workflow input
- variable
- ValueExpr
- ResourceRef
- component binding

### 12.2 图像输入

OCR 的：
- 捕获帧
- OCR ROI
- 模型输入

Studio UI 不再只写一个模糊的 `input`。

---

## 13. Resolved Node Inputs

每个节点开始前保存：

`nodes/<seq>_<node>/resolved-inputs.json`

示例：

```json
{
  "node_id": "find-group",
  "fields": {
    "group_name": {
      "source": "workflow_input",
      "value": "ArgusFlow 测试群"
    },
    "target": {
      "source": "expression",
      "value": "text(exact=$group_name)"
    },
    "application": {
      "source": "resource_ref",
      "producer_node_id": "open-wechat",
      "output_name": "session"
    }
  }
}
```

secret：
```json
{"source":"workflow_input","redacted":true,"value":null}
```

---

## 14. OCR 三层图像

### 14.1 Captured Frame

WGC + popup compose 后的完整当前帧。

回答：
- 捕获窗口对不对？
- popup 是否在图里？
- 当时 UI 到底是什么？

### 14.2 OCR Source ROI

Rust `OcrRequest.image`。

回答：
- OCR 被裁到了哪块？
- ROI 是否偏移？
- Dirty/RefreshPlan 是否正确？

### 14.3 Exact Model Input

Python `PreparedOcrImage.pixels`。

回答：
- 放大以后是什么？
- CLAHE 后是什么？
- sharpen 后是什么？
- Paddle 真正看见什么？

这是 OCR Viewer 默认打开的图。

---

## 15. Worker 协议 v4

当前协议 framing 已经支持：

`JSON control + optional binary body`

所以无需 base64。

建议 request：

```json
{
  "diagnostics": {
    "capture_model_input": true,
    "encoding": "png"
  }
}
```

Worker：

```text
BGRA body
 -> RGB
 -> prepare_ocr_image
 -> prepared.pixels
      ├-> pipeline.predict(prepared.pixels)
      └-> lossless PNG encode
```

Response control 描述：
- kind
- encoding
- width
- height
- body_length
- sha256

Response binary body：
`model-input.png`

关键验收：
**debug PNG 解码后的像素必须与 predict() 收到的 ndarray 完全一致。**

---

## 16. 为什么 Worker 不直接写文件

Worker 不接收 Host 任意 destination path。

原因：
- Run 生命周期归 Host。
- Retention 归 Host。
- Path security 归 Host。
- Quota/dedup 归 Host。
- Worker 不需要知道用户目录。

Worker 只返回 bytes + metadata。

---

## 17. OCR Result

保持：
- request_id
- frame_id
- topology_generation
- model
- elapsed_ms
- preprocessing
- raw_text
- confidence
- polygon

Diagnostics 建议增加：
- minimum_score
- preprocess_elapsed_ms
- inference_elapsed_ms
- accepted/rejected reason

这样用户能区分：
“模型没有识别”
与
“识别了但被阈值过滤”。

---

## 18. Scene / Query / Candidate

一次真正影响动作的视觉查询至少产生：

- `scene.json`
- `query.json`
- `candidates.json`
- `selection.json`

`candidates.json` 要能回答：
- OCR 中哪些文本参与了 Query？
- Query 产生几个合法候选？
- 哪些被过滤？
- 为什么被过滤？
- ranking 是什么？
- 最终是 0 / 1 / N？

---

## 19. Top-1 的正确语义

当前 0/1/N 必须保留：

```text
0 -> TargetNotFound
1 -> Unique
N -> AmbiguousTarget
```

不能改成：

```text
N -> confidence max -> click
```

UI 可以显示：
`Candidate #1`

但只有唯一候选才显示：
`Selected Target / 唯一命中`

如果有两个合法候选：
`2 个合法候选，执行已阻止`

这比“最高分自动点”更符合 ArgusFlow 当前确定性设计。

---

## 20. Focus Mask

推荐名称：

`识别焦点 / Focus Mask`

视觉：

- 截图整体轻微压暗。
- Selected Target 区域保持原亮度。
- 区域内叠低透明度主题强调。
- 细边界。
- 标签：`唯一命中 · 96%`。
- hover：raw_text/confidence/backend/frame/request。
- polygon 优先，bbox fallback。

实现：
SVG/Canvas overlay。

不要把 mask 烧进 PNG。
保存 vector `overlay.json`。

---

## 21. Ambiguous Overlay

当 N>1：
- Candidate #1 强调。
- 其他候选弱强调。
- 明确显示 `Ambiguous`。
- 明确显示 `SendInput 未执行`。
- 支持显示全部候选。
- 切换候选只改变 viewer，不改变 Runtime。

---

## 22. WGC 黄色边框

WGC 黄色/彩色描边是：
**系统捕获指示**。

它不是：
- OCR ROI
- OCR box
- Query target
- Click target

产品上拆成：

1. Capture Scope
2. OCR Scope
3. Selected Target

P0 先做 Focus Mask。

P3 可增加：

`CaptureAppearancePolicy::PreferBorderless`

但只能 capability + permission 成功时关闭系统边框。
失败必须继续正常 capture。

---

## 23. Run Inspector

推荐三栏：

```text
Runs           Timeline                    Detail
-------------------------------------------------------
17:50 ✓        搜索群结果                   概要
17:47 ✓ recov   ├ 输入解析                  OCR
17:40 ✕         ├ Capture 86ms             Query
                ├ OCR Small 214ms          Evidence
                ├ AQL Query: 2 candidates  Raw JSON
                └ Failed
```

默认按节点/Span 折叠。

不要把 200 条内部事件平铺。

---

## 24. OCR Viewer

Tab：
1. 捕获帧
2. OCR ROI
3. 模型输入

默认：
`模型输入`

Overlay 开关：
- Selected Target
- Candidates
- OCR boxes
- OCR ROI
- Dirty Regions
- Safe Point
- Coordinates

OCR item table：
`text / confidence / bbox / source`

点击 item：
图中同步高亮。

---

## 25. Root Cause

P0 使用规则归因，不用 LLM 猜。

类别：

- workflow_input
- resource
- planner
- capture
- stability
- refresh
- ocr_input
- ocr_worker
- ocr_result
- scene
- query
- materialize
- revalidation
- actuation
- verification

例如：

`OCR 成功，Query 返回 2 个合法候选，因此按 0/1/N 安全规则拒绝点击。`

---

## 26. TraceLevel

```rust
pub enum TraceLevel {
    Off,
    Basic,
    Diagnostics,
    Forensics,
}
```

Off：
仅 live ExecutionEvent。

Basic：
- manifest
- workflow snapshot
- inputs
- node lifecycle
- errors
- evidence refs

Diagnostics：
额外：
- source frame
- source ROI
- exact model input
- OCR result
- Query candidates
- selection overlay

Forensics：
额外：
- full scene
- DirtyMap
- branch failure trace
- 更完整 planner explain

Studio 推荐默认：
`Diagnostics`

---

## 27. Retention

同时控制：

- max_runs
- max_total_bytes
- TTL

运行中的 Run 永不清理。

支持：
`pinned=true`

示例策略不是产品永久契约：

```text
Diagnostics:
  max_runs = 10
  max_total_bytes = 2 GiB
  ttl = 24h
```

---

## 28. 隐私

截图和 OCR text 均视为 sensitive。

要求：
- 默认只在本机。
- 不自动上传。
- secret input 不落明文。
- protected/password evidence 脱敏。
- export 显式操作。
- export 可排除截图。
- 可删除单 Run。
- 可清空历史。
- 前端只传 run_id/artifact_id，不传任意路径。

---

## 29. RunTraceWriter

推荐：

```text
producer
  -> bounded channel
  -> single writer
       -> events.jsonl
       -> artifacts
```

事件分级：
- Critical
- Normal
- Verbose

Critical 不静默丢。

图片永远不进 JSONL。

trace IO 失败：
- 不覆盖 AutomationError。
- `trace_degraded=true`
- 尽量记录 persist failure。

---

## 30. 为什么 Trace IO 不能成为业务 fatal

当前 `ExecutionEventSink::emit()` 返回错误会终止 Run。

因此：
实时 UI delivery
与
trace persistence

必须拆分失败语义。

磁盘满了不应该让本来成功的自动化动作变成：
`WorkflowFailed`

Trace 是 best effort observability。
业务错误必须保真。

---

## 31. Crash Recovery

- JSONL append。
- 关键事件 flush。
- artifact `.tmp` + rename。
- manifest `.tmp` + rename。
- 启动扫描遗留 running。
- 修复成 crashed。
- JSONL 半行尾部忽略。
- UI 标注日志可能不完整。

---

## 32. Evidence 与 Run Trace

不是同一个类型。

正确关系：

```text
Run
  ├ Trace
  ├ Artifacts
  └ Evidence Bundles
```

Evidence 继续：
- fallback 前 capture
- best effort
- preserve original AutomationError

只是把 sink root 收敛到：

`<run>/evidence/`

---

## 33. 成功 Run 也要保存

理由：

很多退化首先表现为：
- Tiny 失败，Small 成功。
- 主 selector 失败，fallback 成功。
- OCR 置信度下降。
- Full OCR 频率升高。
- Workflow 仍然成功但耗时变长。

只存失败就看不到这些趋势。

Timeline 应显示：
`recovered`

---

## 34. 性能指标

至少：

- capture_wait_ms
- stable_frame_ms
- dirty_compute_ms
- ocr_queue_ms
- ocr_preprocess_ms
- ocr_inference_ms
- ocr_transport_ms
- scene_merge_ms
- query_ms
- materialize_ms
- revalidation_ms
- action_ms
- verification_ms

优先复用已有 Vision metrics。

---

## 35. 坐标系

每个 rect 必须声明：

- frame_local_physical
- ocr_model_input
- surface_local_physical
- virtual_screen_physical

必须保存：

- ROI offset
- preprocessing input/output size
- inverse transform
- frame bounds
- surface bounds
- screen bounds
- safe point
- dpi
- screen origin

这样才能区分：
“OCR 对，点击 transform 错”
和
“OCR 本身就错”。

---

## 36. Frontend 状态

当前：
`events + runId + running`

建议拆：

- activeRunId
- activeRunLiveEvents
- selectedRunId
- selectedTraceEventId
- selectedArtifactId
- runList
- summaryCache

历史 Run 分页读取。

---

## 37. Tauri Run API

建议：

```text
list_runs
get_run
read_run_events
list_run_artifacts
read_run_artifact
delete_run
pin_run
export_run_bundle
open_run_directory
get_trace_settings
set_trace_settings
```

大图片不要 JSON byte array。

后端按 artifact_id 做安全资源解析。

---

## 38. Export Bundle

`argusflow-run-<run_id>.zip`

包含：
- manifest
- summary
- workflow snapshot
- events
- artifacts
- evidence
- README

README 自动列：
- app version
- repo revision
- vision protocol
- worker version
- PaddleOCR version
- failed node
- root cause
- 关键 artifact

支持：
`不包含截图`

---

## 39. Runtime 改造

`WorkflowEngine::start_with_components`：
- 创建 RunTraceSession。
- 固化 workflow/components/input。
- 创建 manifest。
- 再 spawn execute。

`execute()`：
- 更新 running/final status。
- finalize summary。

Node：
- started 前 resolved inputs。
- outputs 后保存摘要。

Runtime 不依赖 Tauri。

---

## 40. Tauri Host 改造

`TauriEventSink`
升级为 Host composite sink。

Live：
继续 emit。

Trace：
best effort。

新增：
RunStore / ArtifactStore 到 AppState。

---

## 41. PreparedPlan 改造

记录：
- candidate attempt
- backend
- branch_path
- failure classification
- fallback
- recovered branch
- evidence ref

不要从 Display string 反解析。

---

## 42. Vision 改造

复用现有 `SceneExecutionTrace`。

把它从：
“timeout 才保存 last input”

升级为：
“可选绑定 RunTraceContext 的 scene span”

instrument：
- cache
- capture
- stable frame
- dirty
- refresh
- OCR request
- worker
- scene merge

---

## 43. OCR Worker 改造

协议 v4。

Diagnostics：
- exact model input PNG
- preprocessing timing
- inference timing
- threshold metadata

Basic：
不做 PNG debug encode。

Worker 不写 Host 路径。

---

## 44. WGC 改造

P0：
只记录 capture provenance。

P3：
尝试 PreferBorderless。

borderless 不可用时：
正常 capture。

不要把 borderless 当 OCR 调试前置条件。

---

## 45. Visual Resolver 改造

记录：
- Cache/Tiny/Small/Medium stage
- selected scene
- candidate count
- confidence gate
- mapped bounds
- safe point
- pre-input revalidation
- stale rejection

当前 0.80 点击门槛必须在 trace 可见。

---

## 46. 历史 Run 与画布

历史 Run 使用自己的 Workflow Snapshot。

点击 timeline node：
如果当前画布还有同 node_id，可定位。

没有：
显示
`该节点已从当前工作流删除`

历史状态不覆盖当前 live runState。

---

## 47. 结构化错误

不要只存：
`error.to_string()`

推荐：

```json
{
  "code": "ambiguous_target",
  "backend": "ocr_small",
  "query": "...",
  "matches": 2,
  "retryable": false,
  "fallback_allowed": false,
  "message": "..."
}
```

---

## 48. 一个理想的失败体验

Workflow 失败。

Timeline：

```text
搜索群结果
  Capture          ✓ 86ms
  OCR Small        ✓ 214ms
  AQL Query        ✕ 2 candidates
  SendInput        未执行
```

右侧：
模型输入。

图上：
两个候选。

顶部：

`OCR 识别成功；失败发生在目标唯一性判断。`
`发现 2 个合法候选，因此拒绝点击。`

开发者马上知道：
不是 PaddleOCR bug。

---

## 49. 另一个理想失败体验

Timeline：

```text
OCR Small        ✕ 0 items
```

模型输入一看：
ROI 只截到了空白。

Root Cause：

`OCR input region does not contain target UI`

开发者马上去查：
RefreshPlan / ROI / scene coverage。

---

## 50. 不建议的方案

- 只多打印 OCR text。
- 继续覆盖 last-ocr-input。
- 把图片扔一个 global folder。
- 只保存 WGC screenshot。
- 只保存 model input。
- 用黄色 WGC border 当 OCR visualizer。
- Ambiguous 时自动取最高分。
- base64 图片进 JSONL。
- Worker 直接写 run directory。
- 先上大型远程 tracing 平台。

---

## 51. P0 / P1 / P2 / P3

### P0：Run 可追踪

- Run directory
- manifest
- events.jsonl
- workflow snapshot
- resolved inputs
- Run History API
- Studio Run Timeline

### P1：OCR 完整输入链

- source frame
- source ROI
- protocol v4
- exact model input
- OCR result
- OCR Viewer

### P2：Query + Focus Mask

- candidates
- selection
- overlay
- ambiguous viewer
- safe point
- coordinate inspector

### P3：Forensics

- run-scoped Evidence
- retention
- pin
- export
- crash recovery
- perf waterfall
- optional borderless WGC

---

## 52. PR 拆分

1. PR-1 RunTraceStore skeleton
2. PR-2 Workflow snapshot + resolved inputs
3. PR-3 Run Tauri APIs
4. PR-4 Studio Run History
5. PR-5 Vision TraceContext
6. PR-6 OCR frame/ROI artifacts
7. PR-7 Vision Protocol v4 exact model input
8. PR-8 OCR Viewer
9. PR-9 Query candidate trace
10. PR-10 Focus Mask
11. PR-11 Evidence run scope
12. PR-12 retention/pin/export
13. PR-13 crash recovery
14. PR-14 optional borderless WGC

---

## 53. Definition of Done

- 每次 Studio Run 有稳定目录。
- Run ID 与 Runtime 一致。
- 可查看执行时 Workflow Snapshot。
- 可查看最终 resolved node inputs。
- 可查看 OCR source ROI。
- Diagnostics 可查看 exact model input。
- 可查看 OCR raw result。
- 可查看 Query candidates。
- 唯一命中可查看 Focus Mask。
- Ambiguous 显示全部合法候选且不点击。
- 可查看 frame/surface/screen bounds。
- 可查看 safe point。
- 可确认 SendInput 是否发生。
- 可确认 Verification outcome。
- Evidence 从同一 Run 打开。
- recovered fallback 可见。
- Run 可导出/删除/pin。
- secret 不落明文。
- crash 后仍可读。
- trace IO 失败不改变 AutomationError。
- 现有中文 ExecutionLog 不回归。

---

## 54. 一句话产品定义

完成后，ArgusFlow 的每次运行都应该像一次可调试测试：

你可以打开它，
看到输入，
看到每一步，
看到 OCR 真正吃到的图，
看到 OCR 返回的框，
看到 AQL 为什么选中/没选中，
看到最终点击区域，
看到 Failure Evidence，
最后一键导出整个现场。

做到这一点后，
“我甚至不知道 OCR input 是什么”
应该从产品层面消失。

## 55. 1500 行版实施附录

- [ ] Runtime：`run_id` 必须与 Run 目录、Manifest、ExecutionEvent、TraceEvent 完全一致。
- [ ] Runtime：节点执行前必须保存最终 `resolved-inputs.json`，并记录输入来源。
- [ ] Runtime：Secret 输入不得以明文或稳定低熵 hash 落盘。
- [ ] Runtime：Trace 持久化失败不得替代原始 `AutomationError`。
- [ ] Tauri：实时事件继续驱动画布；历史 Run 由独立 RunStore 读取。
- [ ] Tauri：前端只能按 `run_id / artifact_id` 读文件，不接受任意磁盘路径。
- [ ] Studio：Run History 与 active run 状态必须分离，历史状态不得污染当前画布。
- [ ] Studio：默认 Timeline 按 Node/Span 折叠，raw trace 仅在开发者展开时显示。
- [ ] Vision：保留现有 `SceneExecutionTrace`，扩展为可选 RunTrace sink，不另起一套 Scene Runtime。
- [ ] Vision：StableFrame、DirtyMap、RefreshPlan 都必须产生可关联的结构化 trace。
- [ ] OCR：每个 request 必须可追到 `request_id / frame_id / topology_generation / roi / model`。
- [ ] OCR：必须保存 Worker 前 `source-roi`，用于判断裁剪与刷新计划是否错误。
- [ ] OCR：Diagnostics 模式必须保存真正 `pipeline.predict()` 使用的 `prepared.pixels`。
- [ ] OCR：Exact Model Input 必须通过 Golden Test 证明与 `predict()` ndarray 逐像素一致。
- [ ] OCR：Result 必须保留 `raw_text / confidence / polygon`，禁止静默语言纠错后冒充事实。
- [ ] OCR：低于阈值的候选需要可解释 rejection reason，避免把“低置信度”伪装成“完全没识别”。
- [ ] Worker：协议升级优先使用现有 JSON control + binary body，不把图片做 base64。
- [ ] Worker：只返回 debug bytes/metadata，不接收 Host 任意输出路径。
- [ ] Worker：Basic 模式不额外 PNG encode；Diagnostics/Forensics 才生成 model-input artifact。
- [ ] Scene：保存 scene/frame/topology/coverage provenance，禁止历史 bbox 写回 WorkflowDefinition。
- [ ] Query：保存 AQL source、绑定参数、candidate set 和最终 0/1/N outcome。
- [ ] Query：多个合法候选时必须保持 `AmbiguousTarget`，不得 highest-score-wins。
- [ ] Query：`MIN_CLICK_CONFIDENCE=0.80` 的 gate 结果必须在 Run Inspector 可解释。
- [ ] Focus Mask：唯一命中才显示 `Selected Target / 唯一命中`。
- [ ] Focus Mask：Ambiguous 时只显示 `Candidate #1/#2...`，并明确 `SendInput 未执行`。
- [ ] Focus Mask：polygon 优先、bbox fallback，overlay 采用 SVG/Canvas 矢量层。
- [ ] Focus Mask：Viewer 缩放、平移、HiDPI、多显示器负坐标下必须保持对齐。
- [ ] WGC：黄色/彩色系统边框只表示 Capture Scope，不得再作为 OCR 命中可视化。
- [ ] WGC：Borderless 仅作为后续 `PreferBorderless` 能力，授权/系统不支持时 graceful fallback。
- [ ] Input：Materialized Target 必须记录 frame/surface/screen bounds 与 safe point。
- [ ] Input：SendInput 前 revalidation 失败时必须明确记录“动作未发生”。
- [ ] Verification：Confirmed/Rejected/Uncertain/OutcomeUnknown 必须在同一 Run 因果链中可见。
- [ ] Evidence：继续复用现有 Evidence Bundle，并把文件根目录收敛到当前 Run。
- [ ] Evidence：Fallback 后恢复成功的 BranchFailure 也必须能在历史 Run 中看到。
- [ ] Retention：同时限制 max runs、max bytes、TTL；active/pinned Run 不自动删除。
- [ ] Privacy：截图、OCR result、model input 默认标记 sensitive，只保存在本机。
- [ ] Export：支持完整诊断包与“排除截图”的脱敏诊断包。
- [ ] Crash：遗留 `running` Run 在下次启动时修复为 `crashed`，JSONL 半行尾部可容错。
- [ ] Performance：图片不进入 events.jsonl；Run List 首屏只读 Manifest/Index。
- [ ] Performance：Artifact 写入使用有界队列，Retention 清理不得阻塞当前自动化。
- [ ] Testing：覆盖 No-op/Upscale/CLAHE/Sharpen 四类 exact-input Golden Test。
- [ ] Testing：覆盖 0/1/N、0.79/0.80 confidence gate、stale revalidation。
- [ ] Testing：覆盖 125%/150%/200% DPI、Owned Popup、负屏幕坐标。
- [ ] PR 顺序：先 Run Trace，再 Exact OCR Input，再 Query/Focus Mask，最后才是 Borderless WGC。
- [ ] 最终验收：一次失败 Run 无需重跑即可判断问题属于 Capture、ROI、Preprocessing、OCR、Scene、Query、Transform、Actuation 或 Verification。
- [ ] 架构红线：任何可观测性改造都不得改变 ArgusFlow 当前 Planner、Fallback、0/1/N 与非幂等动作安全语义。

**最终原则：Run Trace 记录事实，Focus Mask 解释事实；两者都不能替代或篡改执行语义。**
