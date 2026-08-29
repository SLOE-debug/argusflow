# ArgusFlow 视觉场景索引 + AQL 空间查询重构方案（1500 行核心版）

> 仓库：`SLOE-debug/argusflow`
> 审计基线：`main @ f16b0bb8179df64e7bf7f1812a939289e499d5e3`（2026-08-29）
> 目标：彻底移除微信工作流对固定像素/固定归一化区域的业务依赖，把视觉自动化升级为「完整场景初始化 + Dirty ROI 增量刷新 + 结构化视觉索引 + AQL 空间/关系查询」。
> 结论：用户提出的方向是正确的，但 `ASCII 文本` 应是可读投影而不是执行事实源；AQL 应查询结构化 `VisualSceneIndex`，而不是反解析 ASCII。
> 范围：Windows 桌面视觉路径、微信自绘 UI、AQL、PreparedPlan、VisionRuntime、Evidence、SendInput；不重新设计工作流 Runtime。

## 0. 结论先行

- 当前微信实现严格来说不是“绝对 px offset”，而是写死在模板里的 `NormalizedRect` 比例区域；它比绝对像素略强，但依然把微信某一版布局当成业务事实。
- `WECHAT_SEARCH_OVERLAY_REGION / WECHAT_SEARCH_RESULTS_REGION / WECHAT_HEADER_REGION / WECHAT_MESSAGE_REGION` 应从业务工作流中删除。
- 视觉初始化应以 `AppSession` 的可见 `VisualSurface` 为单位做一次完整 OCR，建立完整 `VisualSceneIndex`。
- 动作后只对 DirtyMap 覆盖区域做 OCR，未变化区域继续复用上一 Scene 的节点；大范围变化、resize、DPI 或 topology 变化时自动升级 Full Refresh。
- 当前仓库已经具备 Scene、DirtyMap、局部 Scene merge、CompactText、SpatialText、SceneDelta 的大部分底座，不需要推倒 OCR 管线。
- 真正应该重构的是“查询语义”：查询不再告诉 OCR 去扫某个微信固定区域，而是先保证场景事实完整，再由 AQL 在结构化 Scene 上筛选、排序、建立空间关系。
- AQL 应新增 `within / nearest / near / left_of / right_of / above / below / same_row / same_column / around / changed_since / added_since` 等查询代数。
- 对于“左上角多大范围”这种明确的用户查询，可以用 viewport 百分比作为查询约束；但这必须是用户显式意图，而不是微信模板私藏的布局常量。
- 对于“以 A 为中心找最近或第二近的 B”，必须用 anchor-relative 空间查询，距离按元素尺寸/viewport 归一化，禁止持久化像素距离。
- 最终点击仍然必须落到物理像素，这是输入设备的事实；但该坐标只能由当前 Scene 的 bbox/row/hit-region 动态物化，不得写进 workflow。
- `ASCII / SpatialText` 保留并强化，用于调试、Evidence、LLM 阅读和 Studio Inspector；AQL 执行必须直接访问 Node/BBox/Index。
- 长期应让 AQL 成为 UIA/CDP/Vision 的统一查询入口，并把现有 `TargetLocator::Visual` 降级为兼容层或后端 IR。

## 1. 审计：你指出的问题到底是不是事实

当前微信模板在 `src/features/workflow/model/wechatTemplateParts.ts` 中直接定义了四个归一化矩形。
```ts
export const WECHAT_SEARCH_OVERLAY_REGION = {
  x: 0.08, y: 0.10, width: 0.38, height: 0.12,
};
export const WECHAT_SEARCH_RESULTS_REGION = {
  x: 0.00, y: 0.00, width: 0.58, height: 0.72,
};
export const WECHAT_HEADER_REGION = {
  x: 0.34, y: 0.00, width: 0.40, height: 0.18,
};
export const WECHAT_MESSAGE_REGION = {
  x: 0.34, y: 0.28, width: 0.66, height: 0.72,
};
```

这些值不是屏幕绝对像素，而是相对 viewport 的比例。
但是从“定位身份”的角度，它们和 px offset 属于同一类技术债：都是预先猜测 UI 出现在某一几何位置。
它们对 DPI 有一定抵抗力，却不能抵抗布局重排、侧栏尺寸变化、微信版本变化、不同窗口宽高比、字体缩放、搜索弹层改版。
当前 `wechatMessage.ts` 把搜索界面确认、群结果查找、群结果点击、群标题确认全部绑定到这些区域。
因此只要文字仍然存在但离开预设区域，Runtime 会给出 TargetNotFound；这不是 OCR 识别失败，而是 selector 设计失败。

现有 1500 行微信方案已经意识到绝对像素不可靠，但它最终仍采用“Header/SearchResults/Editor/Message 的 NormalizedRect”。
这适合作为临时 P0 workaround，不适合作为 ArgusFlow 的长期视觉查询模型。

## 2. 必须先区分：哪些像素是坏的，哪些像素是必需的

- 坏：`群搜索结果固定在左侧 58%`。
- 坏：`群标题固定在 x=0.34~0.74, y=0~0.18`。
- 坏：`消息区域固定在窗口右下 66%`。
- 坏：`文本中心 + 27px` 作为可点击点。
- 坏：工作流持久化一个屏幕绝对坐标并当成元素身份。
- 坏：把 OCR ROI 当成 selector 语义。
- 允许：Diff 的 tile_size 使用 px。
- 允许：Dirty ROI padding 使用 px。
- 允许：OCR polygon/bbox 使用 frame-local physical px。
- 允许：WGC crop 使用 physical rect。
- 允许：最后一刻 SendInput 使用 virtual-screen physical px。
- 允许：输入提交前用目标 bbox 做 dirty revalidation。

一句话：**像素可以存在于感知与执行层，不可以成为业务定位语言。**

## 3. 当前仓库其实已经做对了 60%

- `VisualScene` 已保存 `scene_id / frame_id / topology_generation / viewport / nodes / lines / rows / compact_text / spatial_text`。
- `SceneBuildOptions` 已支持 `base_scene + refresh_regions`，能复用未变化区域节点。
- `VisualSceneCache` 已支持 dirty ROI、分区 freshness、局部替换。
- `compute_dirty_map` 已用低分辨率 tile 做变化检测，并在大变化时升级 full refresh。
- `VisionRuntime::validate_materialized_target` 已在点击前重新抓轻量帧并阻止 stale bbox。
- `VisualSceneDelta` 已能表达 added / removed / changed。
- `NewTextExistsSince` 已经具备高风险动作后“新文本”验证语义。
- `SpatialText` 已经说明 bbox 才是真值，文本只是投影。
- AQL 已有强类型 AST、Any、Not、First、Nth、后端 Compiler/Planner 方向。
- AQL 审计文档已经预告 `nearest / spatial / Vision` 是下一阶段能力模型必须面对的问题。

因此本方案的原则不是“重写视觉”，而是把已有视觉事实层与 AQL 真正接起来。

## 4. 当前真正的架构断点

```text
Workflow
  └─ TargetLocator::Visual
       └─ VisualQueryExpr { text, exact, region }
            └─ SceneRefreshPolicy.normalized_query_region
                 └─ choose_refresh_plan(query_region)
                      └─ OCR 只扫这个区域
                           └─ matching_nodes 再按同一 region 过滤
```

这里把两个完全不同的概念耦合了：
- 查询语义：用户想在什么逻辑范围找目标。
- 刷新语义：哪些像素已经发生变化，需要重新 OCR。

结果是：一个业务 query region 同时决定了“哪里算候选”和“哪里值得重新观察”。
这也是当前第一查询可能只 OCR 一个小区域的根因。
`choose_refresh_plan` 在无 base scene 且 query_region 小于 viewport 时会返回 Partial。
这与“先建立整个 exe/window 的视觉事实，再查询”的目标正相反。

## 5. 新架构一句话

```text
AppSession / WindowSet
        ↓
VisualSurface bootstrap（每个可见 surface 首次完整 OCR）
        ↓
Complete VisualScene
        ↓
VisualSceneIndex
  ├─ TextIndex
  ├─ GeometryIndex
  ├─ Row/Line Index
  ├─ Role/Source Index
  ├─ Temporal Track Index
  └─ Compact/Spatial ASCII Projection
        ↓
AQL Parser / HIR
        ↓
VisionQueryCompiler
        ↓
VisionQueryPlan
        ↓
0 / 1 / N deterministic result
        ↓
Materialize current bbox / hit region
        ↓
SendInput
        ↓
Capture → DirtyMap → Partial OCR → Scene merge → Verify
```


## 6. “整个 exe OCR”应怎样精确定义

不建议真的按 EXE 枚举所有隐藏窗口并 OCR。
正确作用域是 `AppSession` 下当前允许参与交互的可见 VisualSurface 集合。
- Primary window。
- 同一 AppSession 的 Owned Popup。
- 显式归属该会话的 same-process top-level surface。
- 同一 surface 内的 GPU overlay 不单独建窗口，但会通过 DirtyMap 进入增量刷新。
- 不自动包含桌面、任务栏、IME、其他应用、隐藏窗口。

这样既满足“整个 exe 可见 UI 的完整事实”，又不会退化成全桌面 OCR。

## 7. Bootstrap：首次必须建立完整场景

```rust
pub enum ObservationCoverage {
    Empty,
    Partial { covered: Vec<PhysicalRect> },
    Complete,
}

pub struct VisualSurfaceState {
    pub window: WindowIdentity,
    pub topology_generation: TopologyGeneration,
    pub coverage: ObservationCoverage,
    pub scene: Option<Arc<VisualScene>>,
    pub last_stable_frame: Option<Arc<CapturedFrame>>,
}
```

- 新 surface 第一次参与自动化时，默认 `force_full_ocr = true`。
- 只有 `ObservationCoverage::Complete` 的 scene 才允许全局 AQL 查询给出“确定不存在”的结论。
- Partial scene 只能回答显式限定在 covered 区域内的查询。
- 默认产品模式不暴露“首次只扫 query region”的性能捷径。
- 如果未来 benchmark 证明完整 bootstrap 太慢，可以提供高级 `BootstrapMode::Lazy`，但 Explain 必须暴露 completeness。
- Window resize、DPI 改变、拓扑 generation 变化后，旧 Complete 状态立即失效。

## 8. 修改 RefreshPlanner：Dirty 决定刷新，Query 不决定观察事实

当前：
```text
query_region
   ↓
choose_refresh_plan
   ↓
Partial OCR
```

目标：
```text
Scene completeness + DirtyMap + topology + freshness
   ↓
choose_refresh_plan
   ↓
Full / Partial / CacheOnly

AQL query
   ↓
只在已经建立的 SceneIndex 上过滤
```

P0 最重要的改动是把 `query_region` 从默认刷新决策中移除。
如果 AQL 显式写 `within(viewport(...), ...)`，那只是候选过滤，不应该反向导致未观察区域被永久忽略。
可选性能优化：在 Complete base scene 已存在时，AQL 目标涉及的 dirty 子区域可以提高刷新优先级；但不能掩盖其它 dirty 区域的 stale 状态。

## 9. 推荐的刷新状态机

```text
No Scene
  → capture stable full surface
  → Full OCR
  → Complete Scene

Complete Scene + no dirty
  → CacheOnly

Complete Scene + small dirty
  → OCR dirty regions only
  → merge with base scene
  → Complete Scene remains complete

Complete Scene + major transition
  → Full OCR

Resize / DPI / topology mismatch
  → drop geometry continuity
  → Full OCR

Partial/failed bootstrap
  → never claim global TargetNotFound
  → retry full or return ObservationIncomplete
```


## 10. VisualSceneIndex：真正给 AQL 查的东西

```rust
pub struct VisualSceneIndex {
    pub scene: Arc<VisualScene>,
    pub by_text_exact: HashMap<String, SmallVec<[VisualNodeId; 2]>>,
    pub by_text_prefix: TextPrefixIndex,
    pub geometry: SpatialGridIndex,
    pub by_row: HashMap<VisualRowId, SmallVec<[VisualNodeId; 8]>>,
    pub by_line: HashMap<VisualLineId, SmallVec<[VisualNodeId; 8]>>,
    pub by_role_hint: HashMap<RoleHint, SmallVec<[VisualNodeId; 8]>>,
    pub by_track: HashMap<VisualTrackId, VisualNodeId>,
    pub normalized_viewport: NormalizedViewport,
}
```

P0 不必一开始上 R-Tree。
- 微信一屏 OCR 节点量通常远低于上万。
- 先用按 y/x 排序向量 + uniform grid 即可。
- 精确文本使用 HashMap。
- contains/regex 可在经过空间/role 缩小后的候选上 residual filter。
- 以后性能数据证明有必要，再替换为 R-Tree，不改变 AQL 语义。

## 11. ASCII 文本层：保留，但明确它不是数据库

你的“维护一个 OCR 结果 ASCII 文本”这个想法值得保留。
当前仓库已经有 `compact_text` 与 `spatial_text`，应继续加强。
但不能把 ASCII 当成唯一事实源，因为它会损失：bbox、polygon、confidence、source、scene generation、row/line、identity、temporal delta。
推荐把投影做成三种：
```text
compact.txt
  适合日志 / LLM / 快速 grep

spatial.txt
  保留大致二维排列，适合人肉观察

scene.json
  完整结构化事实，AQL 真正执行来源
```

示例：
```text
@scene=104 frame=781 topology=9 window=wechat
@viewport=1180x760 complete=true

[L003][R002]                  网络结果
[L006][R005]  群聊A           最近消息...
[L007][R006]  ArgusFlow测试群  今日天气
[L011][R010]                  ArgusFlow测试群
[L020][R018]                              输入消息...
```

调试文本里可以带 Node token，但不要让用户把 token 持久化成 selector。

## 12. ASCII 如何热更新

- 结构化 Node 合并完成后，先重新 cluster 被影响的 line/row。
- 最简单可靠的 P0：每次 Scene merge 后重新生成整个 compact/spatial 字符串。
- 字符串重建成本通常远低于 OCR，先不要过早优化。
- P1 如果 profiling 显示文本投影成为热点，再维护行级 rope/segment cache。
- Evidence 保存失败瞬间投影即可；成功路径默认不长期持久化敏感文本。

## 13. VisualNode identity：当前还需要再补一层 TrackId

当前 `stable_hash` 包含 bbox，因此元素移动后会变成新 ID。
当前 `identity_hash` 不含 bbox，但相同文本重复出现时会冲突。
对于空间查询和动作前后验证，建议新增短期 `VisualTrackId`。
```rust
pub struct VisualTrackId(u64);

pub struct TrackedVisualNode {
    pub track_id: VisualTrackId,
    pub node_id: VisualNodeId,
    pub confidence: f32,
    pub continuity: TrackContinuity,
}
```

相邻 Scene 关联策略：
- 先要求 normalized_text 或文本相似性兼容。
- 再看 bbox IoU。
- 再看 center displacement（按 viewport/文字高度归一化）。
- 再看 row/line 邻域。
- 再看 source/role hint。
- 同文本多候选无法唯一配对时，标记 ambiguous continuity，不强行续 ID。
- TrackId 只在 AppSession 生命周期内有效，不作为持久化 selector。

## 14. AQL 为什么应该接管 Vision 查询

现有架构同时存在：
```text
TargetLocator::Query { AqlQuery }
TargetLocator::Visual { VisualQueryExpr }
```

这意味着 UIA/CDP 走统一 AQL，而视觉还在另一条 `text + exact + region` 专用 DSL 上。
长期这会形成两套 selector 心智。
更合理的目标：
```text
TargetLocator::Query { AqlQuery + bindings }
        ↓
Planner
  ├─ UIA Compiler
  ├─ CDP Compiler
  └─ Vision Compiler
```

对于微信 opaque surface，UIA Compiler 会因上下文/能力不可用而落后，Vision Compiler 产生可执行 plan。
用户无需知道“这是 OCR query”。

## 15. AQL v2：新增参数绑定，避免运行时拼字符串

微信群名和消息来自 workflow input。
不要把值插值成 AQL source 字符串。
建议：
```rust
pub struct AqlQueryTemplate {
    pub language_version: QueryLanguageVersion,
    pub source: String,
    pub bindings: BTreeMap<String, ValueExpr>,
}
```

语法：
```aql
text(name = $group_name)
text(name contains $keyword)
```

Parser 把 `$group_name` 解析成 `PredicateValue::Parameter(Symbol)`。
Runtime prepare 阶段解析 ValueExpr，再得到冻结的 Typed Query。
这样避免注入、转义、canonical cache key 漂移，也与当前 ValueExpr 架构一致。

## 16. AQL v2 空间查询代数

建议把空间能力设计成真正的 QueryExpr，而不是给 `text(...)` 塞几个坐标属性。
```rust
pub enum QueryExpr {
    Match { matcher: ElementMatcher },
    Descendant { ancestor: Box<QueryExpr>, target: Box<QueryExpr> },
    Child { parent: Box<QueryExpr>, target: Box<QueryExpr> },
    Any { queries: Vec<QueryExpr> },
    Not { query: Box<QueryExpr> },
    First { query: Box<QueryExpr> },
    Nth { query: Box<QueryExpr>, index: NonZeroUsize },
    Within { area: SpatialAreaExpr, query: Box<QueryExpr> },
    Relative { anchor: Box<QueryExpr>, target: Box<QueryExpr>, relation: SpatialRelation },
    Nearest { anchor: Box<QueryExpr>, target: Box<QueryExpr>, options: NearestOptions },
    AddedSince { query: Box<QueryExpr>, checkpoint: SceneCheckpointRef },
    ChangedSince { query: Box<QueryExpr>, checkpoint: SceneCheckpointRef },
    Css { selector: String },
}
```


## 17. SpatialArea：只表达相对空间，不表达微信布局

```rust
pub enum SpatialAreaExpr {
    Viewport,
    ViewportRect(NormalizedRect),
    Quadrant(ViewportQuadrant),
    AroundAnchor {
        anchor: Box<QueryExpr>,
        radius: RelativeDistance,
    },
    RowOf { anchor: Box<QueryExpr> },
    ColumnOf { anchor: Box<QueryExpr> },
    ClusterOf { anchor: Box<QueryExpr> },
}
```

关键规则：
- `ViewportRect` 可以存在，因为“只看左上 40%”可能就是用户明确要求。
- 模板不得偷偷把“微信群标题”定义成某个 ViewportRect。
- 应用模板优先使用 anchor / row / cluster / temporal relation。
- 所有 NormalizedRect 都是 0..1 相对当前 viewport，绝不使用固定屏幕像素。
- 空间面积只筛选 SceneIndex，不决定初始 OCR 是否完整。

## 18. AQL 示例：只找左上区域

```aql
within(
    area = viewport_rect(x = 0.0, y = 0.0, width = 0.50, height = 0.35),
    query = text(name contains "设置")
)
```

这是用户明确表达“只看左上角”。
它可以跨分辨率变化，但仍然对布局比例有依赖。
因此它适合作为显式限制，不应成为微信内建模板的默认识别方式。

## 19. AQL 示例：A 附近最近的 B

```aql
nearest(
    anchor = text(name contains "网络结果"),
    target = text(name = $group_name),
    direction = below,
    index = 1
)
```

第二近：
```aql
nearest(
    anchor = text(name contains "网络结果"),
    target = text(name = $group_name),
    direction = below,
    index = 2
)
```

这正是用户提出的“以 A 为中心找最近/第二近 B”。
`index` 是显式语义；Runtime 不允许偷偷把多候选选成第一名。

## 20. Nearest 的距离必须与分辨率无关

```rust
pub enum DistanceMetric {
    EdgeGapNormalized,
    CenterDistanceNormalized,
    ReadingOrderDistance,
}

pub enum RelativeDistanceUnit {
    ViewportDiagonal,
    AnchorHeight,
    MedianTextHeight,
}
```

推荐默认 `EdgeGapNormalized`。
物理计算依然使用 bbox px，但最终距离除以 viewport diagonal 或局部 median text height。
AQL 持久化的是“最近”，不是“150px 内”。
如果用户明确需要最大范围，可写相对单位，例如 `max = 4.0 * anchor_height`。

## 21. SpatialRelation 建议集合

```rust
pub enum SpatialRelation {
    LeftOf,
    RightOf,
    Above,
    Below,
    SameRow,
    SameColumn,
    Overlaps,
    Contains,
    Inside,
    Near(RelativeDistance),
}
```

P0 推荐先实现：`LeftOf / RightOf / Above / Below / SameRow / Nearest`。
P1 再加入 SameColumn、Cluster、Near radius、Contains/Inside。
P2 再考虑视觉角色推断和复杂 layout graph。

## 22. 方向判断不要用脆弱阈值

例如 `Below` 不应该定义成 `target.y > anchor.y + 20px`。
可以定义成矩形中心/边界关系：
```text
below(A, B):
  center_y(B) >= center_y(A)
  AND vertical_gap(A, B) >= -overlap_tolerance
  AND horizontal_overlap_ratio(A, B) >= optional_min
```

overlap_tolerance 作为内部布局算法参数，可以用文字高度比例计算。
它不是 workflow selector 常量。

## 23. SameRow 应复用现有 VisualRow

仓库已经有 row clustering。
Vision compiler 对 `same_row(anchor, target)` 首先查 `row_id`。
如果 anchor/target 没有 row_id，才使用几何 residual 判定。
这比写死“搜索结果 y=0.2~0.7”稳定得多。

## 24. Cluster：解决微信“区域”但不写死区域

长期建议在 `VisualLine / VisualRow` 之上增加弱 `VisualCluster`。
```rust
pub struct VisualCluster {
    pub id: VisualClusterId,
    pub bbox: PhysicalRect,
    pub node_ids: Vec<VisualNodeId>,
    pub kind_hint: ClusterKindHint,
    pub confidence: f32,
}
```

Cluster 可以由以下事实推导：
- 文本行密度。
- 连续 row 间距。
- 大块背景/分隔线变化。
- 共同出现/消失的 temporal behavior。
- UIA 外层容器投影（若存在）。
- 未来 GUI grounding。

然后可写：`cluster_of(text(name contains "网络结果")) >> text(name=$group_name)`。
这是真正“搜索弹层”的动态区域，不是固定坐标。

## 25. Temporal AQL：动作之后最好问“什么变了”

微信很多状态其实用时间关系比绝对位置更稳定。
例如点击群结果之前，保存 checkpoint。
点击后 Header 群名通常是新出现或发生位置迁移的同名文本。
```aql
added_since(
    checkpoint = $before_open_chat,
    query = text(name = $group_name)
)
```

或者：
```aql
changed_since(
    checkpoint = $before_open_chat,
    query = text(name = $group_name)
)
```

发送消息同理：
```aql
added_since(
    checkpoint = $before_send,
    query = text(name = $message)
)
```

这比 `WECHAT_MESSAGE_REGION` 更接近“动作是否产生了预期新事实”。

## 26. SceneDelta 目前的一个局限

当前 `VisualSceneDelta` 以 `VisualNodeId` 比较。
而 VisualNodeId 的 stable hash 包含 bbox。
同一文本如果发生位置移动，会被看成 removed + added，而不是 changed。
这对“新文本”有时够用，但对强 temporal 关系不够。
新增 TrackId 后，Delta 应同时输出 node-level 与 track-level 两种差分。
```rust
pub struct VisualTrackDelta {
    pub added_tracks: Vec<VisualTrackId>,
    pub removed_tracks: Vec<VisualTrackId>,
    pub moved_tracks: Vec<TrackMove>,
    pub text_changed_tracks: Vec<TrackTextChange>,
}
```


## 27. VisionQueryCompiler：不要在 Runtime 里 if/else 堆微信规则

```rust
pub trait QueryBackendCompiler {
    fn compile(
        &self,
        query: &ResolvedUiQuery,
        context: &ExecutionContext,
    ) -> Result<QueryPlan, QueryPlanRejection>;
}

pub struct VisionQueryPlan {
    pub root: VisionPlanExpr,
    pub required_indexes: IndexMask,
    pub needs_complete_scene: bool,
    pub temporal_checkpoint: Option<SceneCheckpoint>,
    pub summary: QueryPlanSummary,
}
```

Compiler 负责把通用 AQL 映射为 VisualSceneIndex 操作。
微信组件只保存 AQL source 与绑定，不拥有查询算法。

## 28. Vision Plan IR

```rust
pub enum VisionPlanExpr {
    TextLookup(TextPredicate),
    FilterRoleHint { input: Box<Self>, hint: RoleHint },
    FilterArea { input: Box<Self>, area: ResolvedSpatialArea },
    RelativeFilter { anchor: Box<Self>, target: Box<Self>, relation: SpatialRelation },
    RankNearest { anchor: Box<Self>, target: Box<Self>, metric: DistanceMetric },
    SelectNth { input: Box<Self>, index: NonZeroUsize },
    AddedSince { input: Box<Self>, checkpoint: SceneCheckpoint },
    ChangedSince { input: Box<Self>, checkpoint: SceneCheckpoint },
    Any { branches: Vec<Self> },
    Not { input: Box<Self> },
}
```


## 29. 0 / 1 / N 规则继续保持

- 普通 Query：0 -> TargetNotFound。
- 普通 Query：1 -> Unique。
- 普通 Query：N -> AmbiguousTarget。
- `first(...)`：用户显式允许读取排序后的第一项。
- `nth(...)`：用户显式选择第 N 项。
- `nearest(... index=1)`：按明确距离 metric 排名后选第一，但如果第一名发生不可判定 tie，则 Ambiguous。
- `nearest(... index=2)`：显式选择第二近；必须在 Explain 中输出完整排序。
- Fuzzy 文本最高分永远不能自动覆盖 0/1/N。

## 30. Nearest tie 的安全规则

浮点距离完全相同并不罕见，例如上下对称按钮。
建议：
```text
distance_rank candidates
  ↓
if selected rank has one candidate
  → OK
if selected rank has >1 candidates within epsilon
  → AmbiguousTarget
if index exceeds rank count
  → TargetNotFound
```

不要用 NodeId 作为“业务选择”打破 tie。
NodeId 只用于输出稳定排序和 Evidence。

## 31. 点击物化：不要“文字中心 + offset”

Visual query 返回的是逻辑候选。
动作层再根据当前 scene 生成 hit target。
```rust
pub enum VisualHitTarget {
    TextBounds(PhysicalRect),
    RowBounds(PhysicalRect),
    ClusterBounds(PhysicalRect),
    GroundedBounds(PhysicalRect),
}
```

优先策略建议：
- 如果 AQL 命中 row/list-item 语义，优先 row bounds 的安全内点。
- 如果只命中文字，默认点击文字 bbox 内部安全点。
- 如果 UIA 投影提供可点击外框，优先真实 action bounds。
- 如果需要扩大 hit region，只能由当前 scene 的几何邻域推断，不能固定加 N px。
- commit 前继续使用现有 `validate_materialized_target` 复验 dirty。

## 32. Safe point 算法

```text
candidate rect
  → clamp to owning surface
  → shrink by relative inset (e.g. 10% of rect size)
  → exclude overlapping foreign nodes if possible
  → choose center of largest safe sub-rect
  → client/frame coordinate map
  → virtual-screen physical point
  → SendInput
```

relative inset 是算法参数，不进入 workflow。
无法找到可靠 safe point 时应失败，不猜。

## 33. 微信流程 V3：完全移除固定区域

```text
Start
→ Acquire 微信 AppSession
→ Vision Bootstrap Complete（首次隐式完成）
→ Ctrl+F
→ Wait AQL: text(name contains "网络结果")
→ Ctrl+A
→ TypeText(group_name)
→ Wait AQL: nearest(anchor="网络结果", target=$group_name, below, index=1)
→ Click 上述唯一结果
→ Checkpoint before_open_chat / Scene refresh
→ Verify AQL: changed_since(before_open_chat, text(name=$group_name))
→ TypeText(message)
→ Checkpoint before_send
→ Enter
→ Verify AQL: added_since(before_send, text(name=$message))
→ End
```

没有 SearchResults Rect。
没有 Header Rect。
没有 Message Rect。
所有空间判断来自当前视觉事实和相对关系。

## 34. 微信搜索结果如何更稳

P0：使用“网络结果”作为 anchor，选择其下方最近的 group_name。
P0：若 group_name 出现多个并且距离无法唯一决定，返回 Ambiguous，不点击。
P1：使用 cluster_of(anchor) 只在同一搜索弹层 cluster 内找。
P1：使用 row_of(group_name) 点击整行安全区域。
P2：融合弱 role hint / GUI grounding 判断该行是否具备可交互 list-item 外观。
这个演进路径不需要任何微信坐标常量。

## 35. 群标题确认如何不靠 Header Region

“标题必须在上方 18%”只是视觉先验，不应成为身份。
更稳的证据顺序：
- 点击搜索结果前后 SceneDelta 中出现/移动的 group_name。
- group_name 与聊天内容/输入编辑区的相对拓扑。
- 同名文本中读取顺序更靠上者，仅可作为显式 fallback。
- 如果 UIA/AX 外层能提供容器语义，使用跨后端 AQL relation。
- 最终仍无法唯一确认则停止发送。


## 36. 发送后验证如何不靠 Message Region

现有 `NewTextExistsSince` 思路正确。
改造后：
- 发送前保存完整 scene checkpoint。
- Enter 后等待稳定帧。
- DirtyMap 通常只覆盖输入区和聊天底部；OCR 只刷新这些 dirty 区域。
- Scene merge 后用 Track/identity count 判断 message 是否新增。
- 如果历史已有同文本，必须检测“数量增加”或新 track，而不是 TextExists。
- 如果无法证明新增，返回 Uncertain，不自动再次 Enter。

## 37. Readiness wait 与 OCR refresh 仍然分离

PreparedPlan 的 target_wait 是“目标何时出现”。
StableFrameGate 是“画面何时适合观察”。
Dirty refresh 是“哪些事实需要重新 OCR”。
AQL query 是“在当前事实中找什么”。
四者必须保持独立。

## 38. Action 后刷新策略

```text
Act
→ capture next frame(s)
→ stable gate
→ diff(previousStable, currentStable)
→ invalidate dirty regions
→ OCR only dirty regions
→ merge nodes
→ rebuild/update indexes
→ evaluate postcondition AQL
```

不要求每个动作都“整窗再 OCR”。
第一次完整，之后增量；这正是本方案的核心。

## 39. Major transition 的定义

- 窗口 resize。
- DPI scale 变化。
- 主窗口重建或 HWND/PID identity 变化。
- Owned Popup topology 大变化。
- 全屏/大面积动画结束后 dirty coverage 超阈值。
- 大幅滚动导致大面积内容替换。
- 应用切换主题造成整体像素变化。
- 显式 force_full。

Major transition 时 full OCR 是正确性策略，不是性能失败。

## 40. 对现有 DiffConfig 的态度

`tile_size=32 / roi_padding_px=16 / pixel_threshold=12` 这类值可以存在。
它们是图像算法参数，而不是业务 selector。
需要做的是：
- 集中放在 Vision tuning profile。
- 通过 fixture benchmark 校准。
- 不要散落在微信模板。
- 不要让 AQL 用户看到它们。
- Evidence 中记录实际参数，便于复现。

## 41. AQL 与 Backend Capability

空间 AQL 不代表每个 backend 都必须原生支持。
例如 `nearest`：
```text
Vision: Native / Low-Medium cost
UIA: Hybrid（获取 BoundingRectangle 后本地空间计算）
CDP/AX: Hybrid（获取 node bounds 后本地空间计算）
Raw CSS: Unsupported for portable spatial semantics
```

这正符合现有 SupportLevel / QueryPlan Summary 方向。
能力事实应来自 Backend Compiler，不要再写第二套 analyzer 猜测。

## 42. Context-aware Planner

同一个 AQL：
```aql
nearest(
  anchor = text(name = "用户名"),
  target = textbox(),
  direction = right,
  index = 1
)
```

在 Win32/UIA 可见应用中，UIA 可能最便宜。
在 Chromium 中，CDP/AX 可能最便宜。
在微信 opaque surface 中，Vision 是可执行主路径。
因此不要在 Workflow 写 `visual(...)` 才能使用空间查询。

## 43. TargetLocator 迁移

建议分三步：
- 阶段 A：保留 `TargetLocator::Visual`，新增 `VisionQueryCompiler` 支持 AQL，同时微信 V3 先切到 Query。
- 阶段 B：所有新 Studio 视觉节点只产生 AQL Query，旧 VisualQueryExpr 仅用于 migration。
- 阶段 C：内部仍可有 ResolvedVisualQuery/VisionPlan IR，但持久化 workflow 不再暴露 Visual 专用 selector。

这样不会一次性破坏当前 workflow schema。

## 44. VisualQueryExpr 迁移到 AQL 的映射

```text
{ text: literal("确定"), exact: true, region: none }
→ text(name = "确定")

{ text: $group_name, exact: false, region: none }
→ text(name contains $group_name)

{ text: ..., region: NormalizedRect(...) }
→ within(area=viewport_rect(...), query=text(...))
```

旧 region 能无损迁移，但新的微信组件不再生成这些 region。

## 45. AQL Parser 改动

- Lexer 增加 `$identifier` 参数 token。
- Parser `parse_primary` 增加 within/nearest/relative/added_since/changed_since。
- 支持函数 named argument，或者先用严格 positional grammar；推荐 named argument，可读性高。
- CST/Language Service 同步支持新 token 和函数诊断。
- Formatter 只格式化，不做 nearest/within 语义重排。
- Normalizer 对 named args 输出 canonical 顺序。
- Canonical cache key 包含参数符号名，不包含运行时具体值；Prepared cache 再加 resolved binding hash。

## 46. AQL 类型系统改动

```rust
pub enum QueryArgumentValue {
    Literal(PredicateValue),
    Parameter(QueryParameter),
}

pub struct QueryParameter {
    pub name: String,
    pub expected_type: QueryValueType,
}
```

Semantic analyzer 必须保证 `name = $x` 的 x 最终是 text。
空间 index 必须为正整数。
NormalizedRect 每个值必须有限且范围合法。
Direction 只能取受支持枚举。

## 47. Query checkpoint 不应靠字符串 ID

```rust
pub struct SceneCheckpointRef {
    pub source: ValueExpr,
}

pub struct SceneCheckpoint {
    pub window: WindowIdentity,
    pub topology_generation: TopologyGeneration,
    pub scene_id: SceneId,
}
```

Checkpoint 必须绑定 window/topology。
跨窗口、跨 topology 的 temporal query 返回 Uncertain/PlanRejection，而不是假装可比较。

## 48. VisualScene 完整性元数据

```rust
pub struct ObservationState {
    pub coverage: ObservationCoverage,
    pub fresh_regions: Vec<FreshRegion>,
    pub dirty_regions: Vec<PhysicalRect>,
    pub observed_at: Instant,
}
```

当前 freshness 在 CacheState 中。
建议把“是否完整观测”的语义作为明确可查询状态暴露给 Planner Explain。
不要让 `scene.nodes.is_empty()` 被误解成“界面没有文字”，它可能只是未观察。

## 49. Scene merge 的正确性规则

- Dirty ROI 内旧节点必须先视为 invalid。
- 新 OCR 只替换本次完整覆盖的 dirty fragments。
- ROI 外旧节点保留。
- 局部 OCR 边缘要 padding，避免把一个文本切成两半。
- 重叠 ROI 结果按 source quality/confidence 去重。
- merge 后重新建立 line/row/track/index。
- 如果 OCR request 的 frame_id/topology 已过期，禁止合并。
- 如果局部 OCR 失败，该 dirty 区域保持 dirty，不允许沿用旧文本冒充 fresh。

## 50. DirtyMap 与 query 的新关系

Query 可以告诉 Runtime“我现在最关心哪些节点”。
但 Query 不应该把其它 dirty 区域变成 clean。
推荐两级刷新：
```text
Priority refresh:
  与本次 AQL 候选/anchor 相关的 dirty region 先 OCR

Background-in-the-same-call refresh:
  其余 dirty region 在预算允许时继续 OCR

如果预算不足：
  scene completeness/freshness 必须显式标记
  全局 query 不可给出错误否定
```

P0 可更简单：一次 action 后把全部 dirty regions 都 OCR 完再继续。

## 51. 性能预算建议

- 完整 bootstrap：以 1080p/常见微信窗口为基准，建立 P50/P95。
- 增量动作：统计 dirty coverage、OCR processed pixels、scene merge latency。
- 绝大多数键盘输入动作后，目标是 dirty OCR 像素显著低于整窗。
- 查询执行应远快于 OCR；TextIndex exact 查询应近似 O(1)+候选数。
- Nearest 在数百节点规模内可以直接 O(n)；先不为理论复杂度过度设计。
- ASCII projection 重建目标 < scene merge 的小头成本。
- 点击前 revalidation 继续保持廉价新帧，不重新整窗 OCR。

## 52. 微信组件的 AQL 建议草案

```aql
# 搜索界面出现
text(name contains "网络结果")

# 搜索结果
nearest(
  anchor = text(name contains "网络结果"),
  target = text(name = $group_name),
  direction = below,
  index = 1
)

# 点击后确认会话变化
changed_since(
  checkpoint = $before_open_chat,
  query = text(name = $group_name)
)

# 发送后确认新增消息
added_since(
  checkpoint = $before_send,
  query = text(name = $message)
)
```


## 53. 搜索结果如果“网络结果”文案变了怎么办

AQL resilience 用显式 any(...)，不靠 fuzzy winner。
```aql
nearest(
  anchor = any(
    text(name = "网络结果"),
    text(name = "搜索结果"),
    text(name contains "结果")
  ),
  target = text(name = $group_name),
  direction = below,
  index = 1
)
```

每个 fallback 分支在 Explain/Evidence 中可见。
不要在 worker 内静默猜“哪个文本最像网络结果”。

## 54. 多语言与本地化

- 稳定 native 属性可用时优先 UIA/CDP。
- 纯视觉文本 anchor 的本地化变化用 AQL `any(...)` 显式表达。
- 工作区可提供 locale-specific component bindings。
- 不要用位置常量掩盖文案变化。
- 不要让语言模型自动改写 OCR raw_text 后当成 selector truth。

## 55. DPI / 分辨率 / 多屏

本方案为什么比固定区域更通用：
- AQL 关系依赖 node-to-node geometry，不依赖固定屏幕坐标。
- 距离使用归一化 metric。
- 最终物理坐标由当前 frame/client/screen mapping 计算。
- 多屏只影响 virtual-screen origin，不影响 query semantic。
- resize 后 full rebuild scene，不复用旧 bbox。
- DPI change 后 topology/geometry generation 失效，重新观察。

## 56. 仍然会受布局变化影响吗

会，但失败模式完全不同。
固定区域方案：布局稍微变动就可能直接把正确元素排除。
关系方案：只要“搜索结果仍在搜索 anchor 附近/下面”这种语义关系存在，就能继续工作。
如果产品交互语义本身改变，AQL 明确失败并要求升级组件。
没有任何 selector 能对任意 UI 改版无限通用；目标是把依赖从“像素版式”提升到“语义/拓扑关系”。

## 57. Visual RoleHint 的定位

RoleHint 目前很弱，不应伪装成 OCR 事实。
可以让 Vision compiler 把 `text(...)` 作为主要 P0。
未来通过 layout/UIA projection/grounding 得到 Button/ListItem hint 时，再提高查询能力。
AQL `button(...)` 在纯 OCR 场景如果无法证明 role，应 SupportLevel::Unsupported 或低可信 Hybrid，而不是把任何文字都当按钮。

## 58. 视觉空间查询与 UIA/CDP 统一的真正收益

同一条 AQL 不再关心信息来自哪里。
例如 `same_row(text(name="总计"), text(name=$amount))`：
- UIA 可以从 BoundingRectangle + tree relation 计算。
- CDP 可以从 DOM/AX bounds 计算。
- Vision 可以从 OCR row clustering 计算。

这才是 AQL 作为 Argus Query Language 的价值。

## 59. Studio Inspector 应怎样呈现

- 默认只显示 AQL。
- 显示当前 query 的 portable/backend-specific 状态。
- 显示 PreparedPlan 选择了 UIA/CDP/Vision 哪条路径。
- Vision 命中时显示 Scene ID、节点 bbox、TrackId、anchor、distance rank。
- 可以切换“结构化 Scene / SpatialText / Overlay”。
- 如果 query 使用 viewport_rect，明确标记“几何约束，可能随布局变化”。
- 如果 query 使用 nearest/relative，显示 anchor 与目标连线。

## 60. AQL Editor 补全

- 输入 `near` 建议 nearest/near。
- nearest 内补全 anchor/target/direction/index/max_distance。
- direction 补全 above/below/left/right/any。
- `$` 后补全 workflow input / variable binding。
- Hover 显示 distance metric 定义。
- 诊断 viewport_rect 越界。
- 诊断 nearest index=0。
- 诊断 temporal query 缺少 checkpoint 类型。

## 61. Explain 示例

```text
AQL
  nearest(anchor=text("网络结果"), target=text($group_name), below, 1)

Planner
  UIA: Unavailable (opaque surface)
  CDP: MissingContext
  Vision: Native, Ready

Vision Plan
  1. Exact TextIndex lookup: "网络结果" → 1 node
  2. Exact TextIndex lookup: "ArgusFlow测试群" → 2 nodes
  3. Direction filter: below(anchor) → 1 node
  4. Rank EdgeGapNormalized → unique rank #1

Result
  node=V42 track=T17 row=R6
  bbox=[...]
  click_target=RowBounds
  scene=104 frame=781
```


## 62. Evidence 应增加的文件

- `visual_scene.json`：完整结构化事实。
- `visual_index_summary.json`：索引统计。
- `compact_text.txt`。
- `spatial_text.txt`。
- `aql_plan.json`。
- `spatial_candidates.json`：anchor/target/distance/relation。
- `scene_delta.json`。
- `dirty_map.json`。
- `ocr_regions.json`。
- 失败时 `ocr_overlay.png`。

成功路径默认不要持久化完整截图。

## 63. 错误模型

```rust
pub enum VisualQueryError {
    ObservationIncomplete,
    AnchorNotFound,
    AnchorAmbiguous,
    TargetNotFound,
    TargetAmbiguous,
    SpatialRelationUnsatisfied,
    DistanceRankAmbiguous,
    TemporalCheckpointStale,
    SceneStale,
}
```

最终对外仍映射到统一 AutomationError，但 details 必须保留具体原因。

## 64. 不允许的隐式 fallback

- 找不到 Header 就去窗口顶部 20% 再扫一次。
- 找不到搜索结果就把区域放大到 70%。
- 多个同名群就点第一个。
- OCR 不确定就点击最高 fuzzy score。
- 验证不确定就再发一次消息。
- 当前 query 区域没变化就认为整个 Scene fresh。

## 65. 允许的显式 fallback

- AQL `any(...)`。
- OCR Small → Medium，同一语义 query。
- Visual text → GUI grounding，Planner Explain 可见。
- UIA/CDP → Vision，Backend Plan 可见。
- 完整 scene 缺失 → Full OCR。
- 低置信度 ROI → 扩大当前 dirty ROI 后 Medium OCR。

## 66. 文件级改造总览

- `crates/argusflow-core/src/query.rs` — 新增 Parameter/Within/Relative/Nearest/Temporal QueryExpr 与强类型空间参数。
- `crates/argusflow-core/src/automation.rs` — 让 Query locator 支持 bindings；Visual locator 标记兼容迁移路径。
- `crates/argusflow-query/src/lexer.rs` — 增加参数 token、空间函数必要 token。
- `crates/argusflow-query/src/parser.rs` — 解析 named args、within/nearest/temporal。
- `crates/argusflow-query/src/normalize.rs` — 空间函数 canonicalization。
- `crates/argusflow-query/src/formatter.rs` — 空间函数与参数格式化。
- `crates/argusflow-query/src/capability.rs` — 删除重复视觉能力猜测，依赖 compiler summary。
- `crates/argusflow-vision/src/scene/model.rs` — 加入 observation completeness/index version。
- `crates/argusflow-vision/src/scene/node.rs` — 加入可选 TrackId/track metadata。
- `crates/argusflow-vision/src/scene/cache.rs` — 显式暴露 Complete/Partial/Dirty observation state。
- `crates/argusflow-vision/src/scene/delta.rs` — 增加 track-level delta。
- `crates/argusflow-vision/src/refresh.rs` — 默认刷新规划不再由 query_region 裁剪首次观察。
- `crates/argusflow-vision/src/runtime.rs` — bootstrap full surface，增量 refresh 与 query 解耦。
- `crates/argusflow-vision/src/runtime/scene_refresh.rs` — 完整 scene 后只按 DirtyMap OCR。
- `crates/argusflow-vision/src/query.rs` — 从 VisualQuery text+region evaluator 演进为 VisionQueryPlan executor。
- `crates/argusflow-vision/src/query/compiler.rs` — 新增 AQL → VisionPlan 编译器。
- `crates/argusflow-vision/src/query/spatial.rs` — 空间关系、距离、rank 实现。
- `crates/argusflow-vision/src/query/temporal.rs` — AddedSince/ChangedSince。
- `crates/argusflow-vision/src/index/mod.rs` — 新增 VisualSceneIndex。
- `crates/argusflow-vision/src/index/text.rs` — 文本倒排索引。
- `crates/argusflow-vision/src/index/geometry.rs` — uniform grid/geometry index。
- `crates/argusflow-vision/src/index/tracking.rs` — 相邻 scene TrackId。
- `crates/argusflow-vision/src/layout/rows.rs` — 保留 row clustering，提供 spatial query API。
- `crates/argusflow-vision/src/projection/spatial.rs` — 加入可选 node/row debug token。
- `crates/argusflow-vision/src/verification.rs` — 验证条件接收 AQL/temporal plan，不再依赖固定 region。
- `crates/argusflow-agent/src/...` — PreparedCandidate 能携带 VisionQueryPlan summary。
- `crates/argusflow-windows/src/input/...` — 继续只负责 actuation；消费 materialized hit target。
- `src/features/workflow/model/wechatTemplateParts.ts` — 删除四个 WECHAT_*_REGION 常量。
- `src/features/workflow/components/builtin/wechatMessage.ts` — 组件 V2/V3 改成 AQL anchor/temporal query。
- `src/features/workflow/model/defaultWorkflowTemplate.ts` — 删除区域导出与 region UI。
- `src/features/workflow/model/contracts.ts` — Query bindings / AQL v2 前端契约。
- `src/components/workflow/inspector/...` — 视觉目标统一为 AQL Editor。
- `src/features/aql-editor/...` — 补全 spatial/parameter/temporal language tooling。
- `TODO.md` — 同步 VISION-009/010 实际状态并新增 AQL-SPATIAL 里程碑。

## 67. P0 实施顺序：先解决最脆弱问题

1. 定义 ObservationCoverage，并让微信首次视觉查询前完成 full surface OCR。
2. 修改 refresh planner：base scene 完整后只按 DirtyMap 更新；去掉 query_region 对首次观察的裁剪。
3. 建立 VisualSceneIndex（exact text + geometry + row）。
4. AQL 增加参数 binding。
5. AQL 增加 nearest + direction + index。
6. 实现 VisionQueryCompiler 与 executor。
7. 微信搜索结果改用 anchor + nearest，不再用 SEARCH_RESULTS_REGION。
8. 点击结果继续走 current bbox + SendInput revalidation。
9. 发送后验证去掉 MESSAGE_REGION，使用 scene checkpoint + new text count。
10. 删除微信四个 region 常量并新增分辨率/DPI 回归测试。

## 68. P1 实施顺序：真正做到关系定位

1. 加入 SameRow / SameColumn / Relative QueryExpr。
2. 加入 TrackId 和 track-level SceneDelta。
3. 加入 changed_since / added_since AQL。
4. 加入 VisualCluster 与 cluster_of。
5. 点击 hit target 支持 RowBounds/ClusterBounds。
6. UIA/CDP Compiler 对部分 spatial query 提供 Hybrid plan。
7. Studio Overlay 可视化 anchor、candidate、distance。
8. 建立视觉 fixture 数据集和几何关系测试。

## 69. P2：不阻塞核心方案的高级能力

1. GUI grounding 产生 role/cluster hint。
2. 视觉 region segmentation。
3. 图标/无文字按钮匹配。
4. 复杂 layout graph。
5. 跨滚动文档 Track/History。
6. GPU diff/crop zero-copy。
7. 可学习的 hit-region 预测，但必须输出确定性候选与 confidence。

## 70. P0 建议的新 Rust 类型

```rust
pub struct ResolvedAqlQuery {
    pub query: UiQuery,
    pub parameters: BTreeMap<String, ResolvedQueryValue>,
}

pub struct VisualSceneSnapshot {
    pub scene: Arc<VisualScene>,
    pub index: Arc<VisualSceneIndex>,
    pub observation: ObservationState,
}

pub struct VisionQueryResult {
    pub scene_id: SceneId,
    pub matches: Vec<VisualMatchRef>,
    pub explain: VisionQueryExplain,
}
```


## 71. P0 Scene bootstrap API

```rust
impl VisionRuntime {
    pub async fn ensure_complete_scene(
        &self,
        window: WindowIdentity,
        policy: &SceneRefreshPolicy,
    ) -> Result<Arc<VisualSceneSnapshot>, VisionError>;

    pub async fn refresh_dirty_scene(
        &self,
        window: WindowIdentity,
        policy: &SceneRefreshPolicy,
    ) -> Result<Arc<VisualSceneSnapshot>, VisionError>;
}
```

Query backend 不直接决定 OCR ROI。
它先请求符合 freshness/completeness 的 snapshot，再执行 plan。

## 72. P0 Vision query API

```rust
pub fn execute_vision_query<'a>(
    snapshot: &'a VisualSceneSnapshot,
    plan: &VisionQueryPlan,
) -> Result<Vec<VisualMatchRef<'a>>, AutomationError>;
```

选择唯一目标是上层 cardinality/operation 语义，不埋在 TextIndex。

## 73. P0 nearest evaluator 伪代码

```rust
let anchors = eval(plan.anchor)?;
require_unique_anchor(&anchors)?;
let targets = eval(plan.target)?;
let filtered = targets
    .into_iter()
    .filter(|t| direction_matches(anchor, t, plan.direction))
    .map(|t| (normalized_distance(anchor, t, plan.metric), t))
    .collect::<Vec<_>>();
let ranked = rank_with_ties(filtered);
select_explicit_rank(ranked, plan.index)
```


## 74. P0 微信组件迁移策略

- 组件版本从 1.2.0 升到 2.0.0。
- 旧 1.x component instance 保持可运行，避免打开旧工作流就改变行为。
- 新建组件默认使用 2.0.0。
- Studio 提供“升级组件”并展示行为差异：固定区域 → AQL 空间/时间关系。
- 默认工作流模板直接使用 2.0.0。
- 若旧 VisualQueryExpr migration 能自动转换，可提供一键迁移，但不偷偷覆盖已有版本。

## 75. 必须删除的微信代码

```text
WECHAT_SEARCH_OVERLAY_REGION
WECHAT_SEARCH_RESULTS_REGION
WECHAT_HEADER_REGION
WECHAT_MESSAGE_REGION
DEFAULT_WECHAT_REGIONS
```

如果某些值仅用于截图 debug overlay，也应改名为 diagnostic hint，不能参与定位。

## 77. 单元测试：Scene completeness

- 首次 query 即使包含 viewport_rect，也先建立 Complete scene（默认模式）。
- Partial OCR 失败时全局 query 不返回确定 TargetNotFound。
- Dirty region 刷新后 ROI 外节点保持不变。
- Dirty region 未成功刷新时旧节点不可被视为 fresh。
- Major transition 后不复用旧 Complete geometry。
- Topology generation 变化会清空/重建完整观测。

## 78. 单元测试：AQL spatial

- nearest 唯一最近候选。
- nearest 第二近候选。
- nearest tie 返回 Ambiguous。
- below 排除 anchor 上方同名候选。
- same_row 使用 row_id。
- within viewport_rect 只过滤候选，不改变 scene completeness。
- 参数绑定支持中文、引号、反斜杠，不通过字符串拼接。
- added_since 拒绝历史同文本误报。
- changed_since 拒绝跨 topology checkpoint。

## 81. OCR fixture 数据集

- 每个关键微信状态至少保存匿名化/合成 fixture：初始、Ctrl+F、输入群名、结果出现、群聊打开、发送前、发送后。
- 对同一语义状态生成多个 resize/DPI/layout 变体。
- Fixture 测试输入是 CapturedFrame，不依赖真实微信在线状态。
- 黄金数据同时保存 OCR boxes 与期望 AQL match/anchor/rank。
- 失败 fixture 纳入回归，不只保留成功截图。

## 82. 端到端成功标准

- 不同分辨率无需修改任何微信 region 常量，因为这些常量已不存在。
- 不同 DPI 无需修改任何 offset。
- 搜索结果有同名历史/侧栏文字时，anchor relation 能排除错误候选或明确 Ambiguous。
- 动作后只 OCR dirty regions，统计可证明 processed pixels 下降。
- 发送后不靠固定 message region 也能证明新增消息。
- 任何不确定状态都不会自动重复非幂等发送。
- AQL Explain 能说明为何选择某个候选。

## 85. 隐私边界

- 只捕获 AppSession 可见 surface。
- 默认不抓整个 desktop。
- 默认不把成功截图持久化。
- ASCII/scene evidence 只在诊断策略允许时落盘。
- 敏感工作流可关闭 text evidence 或做脱敏。
- Owned Popup 仍必须验证 PID/session ownership。

## 90. NormalizedRect 仍然应该保留吗

应该。
但保留在两类地方：
- 底层算法/调试显式 crop。
- AQL 用户主动写出的 viewport-relative area constraint。

不再作为微信组件默认身份。

## 91. 最小可落地版本，不要一次做太多

如果只做最小可落地批次，优先做这五件事：
1. 首次 Complete Scene。
2. Dirty-only incremental refresh。
3. AQL 参数绑定。
4. nearest(anchor,target,direction,index)。
5. 微信组件删除固定 region。
这五件做完，用户指出的核心问题就已经被根治。
TrackId/Cluster/复杂 Temporal AQL 可以随后增强。

## 93. 建议的 PR 拆分

- `PR-1` — Vision observation completeness + full bootstrap
- `PR-2` — Refresh planner 与 query region 解耦
- `PR-3` — VisualSceneIndex + geometry helpers
- `PR-4` — AQL v2 parameter binding
- `PR-5` — AQL nearest/relative AST + parser + formatter
- `PR-6` — VisionQueryCompiler + executor
- `PR-7` — WeChat component v2 no fixed regions
- `PR-8` — Temporal verification without message/header region
- `PR-9` — Studio spatial Explain/Overlay
- `PR-10` — Resolution/DPI/layout fixture suite

## 102. 关键代码审查规则

- 任何新增 `WECHAT_*_REGION` 直接要求重新论证。
- 任何工作流 JSON 出现物理 x/y click 坐标必须说明为何不能语义定位。
- 任何 `+ 20px` 用于“找到元素附近”都应被拒绝。
- Diff/OCR 图像算法内部 px 参数不受此禁令。
- 任何 fuzzy winner 自动点击必须拒绝。
- 任何 non-idempotent verification uncertain 后 retry 必须拒绝。
- 任何 query 导致未观察区域被当成 clean 必须拒绝。
- 任何 AQL Vision 能力在 UI 层单独实现一套 planner 必须拒绝。

## 120. 最终架构边界表

| 层 | 允许使用 px | 是否持久化业务定位 | 主要职责 |
|---|---:|---:|---|
| Capture | 是 | 否 | WGC/DXGI 帧与坐标 |
| Diff | 是 | 否 | DirtyMap |
| OCR | 是 | 否 | text/polygon/confidence |
| Scene | 是（事实） | 否 | 结构化视觉快照 |
| SceneIndex | 内部可用 | 否 | 文本/空间/row/track 索引 |
| AQL | 默认否 | 是 | 语义/关系/相对空间查询 |
| Planner | 否 | 否 | 选择 backend/plan |
| Materialize | 是 | 否 | 当前 bbox → hit target |
| SendInput | 是 | 否 | 最终物理输入 |
| Workflow | 否 | 是 | 业务语义与绑定 |

## 121. 对用户原始方案的逐条评价

| 用户想法 | 评价 | 本方案修正 |
|---|---|---|
| 最开始整个 exe OCR | 赞同 | 精确为 AppSession 可见 VisualSurface 首次 Complete OCR。 |
| 一步操作后热更新变动区域 | 强烈赞同 | 沿用 DirtyMap + base_scene merge，并与 query region 解耦。 |
| 其它区域不动 | 赞同但需 freshness | 只有未 dirty 的节点可直接复用。 |
| 维护 OCR ASCII 文本结果 | 赞同作为投影 | 结构化 Scene 才是真值；ASCII 用于调试/LLM/Evidence。 |
| AQL 限制左上区域 | 赞同 | 显式 within(viewport_rect)，不作为应用隐藏常量。 |
| A 附近最近/第二近 B | 强烈赞同 | 正式加入 nearest/relative Query Algebra。 |
| px offset 很差 | 方向正确 | 业务定位禁止 offset；图像/输入层仍允许 px。 |
| 换分辨率/屏幕要通用 | 可显著改善 | 相对关系 + normalized metric + current bbox materialization。 |

## 129. 反模式清单

- ``if wechat { region = ... }``
- ``bbox.center + constant_offset``
- ``first OCR match wins``
- ``highest confidence wins``
- ``query ROI == observation ROI``
- ``partial scene == complete scene``
- ``ASCII parse back to coordinates``
- ``message send verification = TextExists``
- ``uncertain → retry Enter``
- ``DPI scale fix = multiply old coordinates``
- ``UI layer decides Vision backend``
- ``worker selects click target``

## 141. 为什么本方案更接近“通用 RPA”

因为模板表达的是关系：
```text
“在搜索结果 anchor 下方找到目标群”
“点击后确认目标群相关事实发生变化”
“发送后确认出现新的消息事实”
```

而不是：
```text
“去左边 58% 找”
“去顶部 18% 找”
“去右下 72% 找”
```


## 144. 核心验收 Gate

- **G1**：代码中不存在微信固定识别区域常量。
- **G2**：首次视觉 surface 有 Complete Scene invariant。
- **G3**：动作后 OCR 仅由 DirtyMap 驱动。
- **G4**：AQL 能执行 nearest(... index=1/2)。
- **G5**：微信搜索结果使用 anchor-relative query。
- **G6**：header/message 验证不需要固定 region。
- **G7**：不同分辨率/DPI fixture 无需改 query。
- **G8**：Ambiguous 永远不自动点击。
- **G9**：Uncertain 发送验证永远不重发。
- **G10**：Explain 能复现 query 选择路径。

## 145. 直接落地的首批 TODO

- [ ] 01. 新增 `ObservationCoverage`。
- [ ] 02. 新增 `ensure_complete_scene()`。
- [ ] 03. 改 `choose_refresh_plan()`。
- [ ] 04. 新增 `VisualSceneIndex`。
- [ ] 05. 新增 geometry normalized distance helper。
- [ ] 06. AQL lexer 参数 token。
- [ ] 07. AQL AST nearest。
- [ ] 08. AQL parser nearest。
- [ ] 09. AQL formatter/normalize nearest。
- [ ] 10. VisionQueryCompiler。
- [ ] 11. 微信组件 v2。
- [ ] 12. 删除 WECHAT_*_REGION。
- [ ] 13. 新增分辨率矩阵 fixture tests。

## 148. 最终技术判断

你的核心判断成立：**把微信 UI 识别建立在预设几何区域上，不足以成为通用视觉自动化。**
当前实现已经从绝对 px 进化到 normalized ROI，但仍然停留在“版式先验 selector”。
ArgusFlow 现有 DirtyMap + VisualScene + AQL 的底座，恰好足以再向前一步。
最有价值的重构不是换 OCR 模型，而是把视觉系统变成一个持续维护的可查询场景数据库。
AQL 则从“跨 UIA/CDP 的语义 selector”升级成“跨语义树与视觉场景的统一查询代数”。

## 149. 一句话架构口号

```text
OCR 负责把像素变成事实；
Scene 负责把事实持续维护；
AQL 负责描述关系；
Planner 负责选择怎么查；
SendInput 只负责最后一厘米的物理动作。
```

> 审计来源均来自当前仓库 `main` 的 docs、AQL、VisionRuntime、Scene/Cache/Diff/Verification 与微信组件实现。
---
**最终原则：业务层描述“是什么、与谁有什么关系”；像素层只负责“现在它具体在哪里”。**
**当微信换分辨率、换 DPI、换窗口比例时，应该重新观察 Scene，而不是重新标定模板。**
