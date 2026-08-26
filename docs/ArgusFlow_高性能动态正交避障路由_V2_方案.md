# ArgusFlow 高性能动态正交避障路由 V2 方案

> 适用仓库：`SLOE-debug/argusflow`  
> 基线：`main` / `fe301cb879775f6f9227c032fa84f9b800d3f1f2`  
> 目标：彻底解决“修改 target 锚点后连线消失”“拖动节点时不能稳定自动避障”“move 过程中全图路由开销过大”三个问题。  
> 本方案只修改通用 `src/flow` 画布内核，不把工作流业务逻辑反向塞进 Flow 层。

---

## 结论先放前面

当前路由器不应该继续沿着：

```text
简单 L/Z 候选
    ↓
失败
    ↓
20px 固定网格 A*
    ↓
失败
    ↓
route = null
    ↓
整条边不渲染
```

这条路线继续打补丁。

我建议直接升级为：

```text
                ┌────────────────────────────┐
                │  Port-aware Route Engine   │
                │  显式 source/target side   │
                └─────────────┬──────────────┘
                              │
                    route fingerprint
                              │
                 ┌────────────▼─────────────┐
                 │ Incremental Dirty Router │
                 │ 只处理真正受影响的边      │
                 └───────┬───────────┬──────┘
                         │           │
                  move 热路径         │ idle / drag end
                         │           │
              ┌──────────▼───┐   ┌──▼────────────────┐
              │ Fast Repair  │   │ Sparse Exact OVG  │
              │ 复用旧路径    │   │ 稀疏正交可见图 A*  │
              └──────┬───────┘   └──┬────────────────┘
                     │              │
                     └──────┬───────┘
                            ▼
                  Always-visible Route Cache
                            │
                            ▼
                        SVG render
```

核心原则只有四条：

1. **边永远不能因为算法失败而从 UI 中消失。**
2. **用户指定 `target.side = 'left'` 后，左侧入口是硬约束，不是建议。**
3. **move 每帧禁止全节点扫描 + 全边重算 + 全图 Worker 路由。**
4. **拖动时优先修补旧路径；只有修补失败的少量边才进入局部寻路。**

最终算法不是“每帧找一条全新的最短路”，而是：

> **稳定复用旧路 → 局部修补 → 局部稀疏寻路 → 拖动结束后异步精修。**

这才适合编辑器交互。

---

# 1. 当前代码为什么会出现“target 改左侧，线直接没了”

我看了当前这几个文件：

```text
src/flow/routing.ts
src/flow/routingPathfinder.ts
src/flow/routing.worker.ts
src/flow/useEdgeRoutes.ts
src/flow/FlowEdges.tsx
src/flow/useCanvasPointerInteractions.tsx
src/flow/spatialIndex.ts
src/flow/routing.test.ts
src/flow/useEdgeRoutes.test.tsx
```

现在不是一个单点 bug，而是 **缓存失效、失败语义和路由算法三层一起有问题**。

---

## 1.1 `useEdgeRoutes` 的 dirty 判定没有把“端点/锚点变化”算进去

现在实时预览主要比较的是节点：

```ts
const changedNodeIds = new Set(
  nodes.flatMap((node) => {
    // position / size 是否变化
  }),
);
```

之后如果已经存在 exact route，而且 source / target 节点没有移动：

```ts
if (
  exact
  && !changedNodeIds.has(edge.source.nodeId)
  && !changedNodeIds.has(edge.target.nodeId)
) {
  return [exact];
}
```

问题是：

```text
target.nodeId 没变
target node position 没变
target node size 没变

但是：

target.side:
right -> left
```

这其实已经是 **路由拓扑变化**，但当前逻辑没有把它识别为 dirty。

而工作流 reconnect 最终确实会把：

```ts
[endpoint]: { nodeId, side }
```

写回 edge。

所以第一层必须修：

> **路由缓存的身份不能只看 node geometry，必须看 edge endpoint fingerprint。**

---

## 1.2 当前把“寻路失败”编码成 `null`，然后 UI 直接把边删掉了

当前核心契约是：

```ts
routeEdge(...): RoutedEdge | null
previewEdgeRoute(...): RoutedEdge | null
```

然后 Hook 中：

```ts
const route = previewEdgeRoute(...);
return route ? [route] : [];
```

Worker 中也是：

```ts
edges.flatMap(
  (edge) => routeEdge(edge, nodes, undefined, index) ?? []
)
```

也就是说：

```text
寻路失败
  ≠
显示一条降级路径 / 保留旧路径 / 显示错误状态

而是：

寻路失败
  =
这条 edge 从 routedEdges 消失
  =
FlowEdges 根本没有 SVG path 可以 render
```

所以截图里的“线没了”并不是偶然视觉问题。

**当前系统就是把 router failure 当成 edge disappearance。**

这个契约必须先改，甚至比换 A* 更优先。

---

## 1.3 当前 20px 固定网格 A* 不适合作为编辑器 move 热路径

现在 `routingPathfinder.ts` 是：

```text
GRID_SIZE = 20
MAX_EXPANSIONS = 20_000
4 邻域
Manhattan heuristic
TURN_COST
```

这套算法离线算一条路没有问题。

但编辑器拖动是另外一种负载：

```text
pointermove
  ↓
requestAnimationFrame
  ↓
nodes 产生新数组
  ↓
preview
  ↓
若简单候选失败
  ↓
重新建障碍索引
  ↓
固定网格 A*
```

网格 A* 的代价主要取决于：

```text
搜索区域面积 / GRID_SIZE²
```

而不是：

```text
真正相关的障碍物数量
```

举例：

```text
两个节点相距 2000px
附近其实只有 4 个障碍物
```

固定网格仍然可能展开大量空白格。

对于画布路由，这是明显浪费。

---

## 1.4 move 预览仍然存在 O(N) 级全节点扫描

`previewEdgeRoute()` 现在会：

```ts
const routingObstacles = routeObstacles(nodes, source.id, target.id);
```

也就是每条需要预览的 edge 都先从全部 nodes 建一遍 rect 列表。

简单候选检测又是：

```ts
obstacles.some(...)
```

失败后：

```ts
findGridPathAgainstRects(...)
```

里面还会再给这些 rect 建一个临时 `SpatialHash`。

于是拖动一帧可能出现：

```text
dirty edge 1
    └─ 扫全部 N nodes
dirty edge 2
    └─ 扫全部 N nodes
dirty edge 3
    └─ 扫全部 N nodes
...
```

节点一多，这个模型一定开始抖。

---

## 1.5 Worker 虽然做了背压，但仍然在“全图重算”

当前 Worker 协议每次拿：

```ts
{
  revision,
  nodes,
  edges,
}
```

然后：

```text
createRoutingIndex(all nodes)
routeEdge(edge 1)
routeEdge(edge 2)
...
routeEdge(edge E)
```

`useEdgeRoutes` 已经做了：

```text
1 个 in-flight
+
1 个 latest pending
```

这个背压设计是对的。

但是它只解决：

> 队列不会无限增长。

没有解决：

> 每次任务还是整张图重算。

拖一个节点时，正常情况下真正需要改变的可能只有：

```text
这个节点相邻的 2~6 条边
+
被这个节点移动路径扫到的 0~几条边
```

不应该重算 700 条。

---

# 2. 先定义不可破坏的路由不变量

这次不要先写算法，先把契约定死。

---

## 2.1 不变量 A：Edge 永远可见

禁止：

```ts
RoutedEdge | null
```

作为最终 UI 路由语义。

建议改成判别联合：

```ts
export type RouteQuality =
  | 'fast'
  | 'exact'
  | 'emergency';

export type RouteFailureReason =
  | 'blocked_port'
  | 'overlapping_nodes'
  | 'search_budget_exceeded';

export type RouteResult =
  | Readonly<{
      kind: 'routed';
      route: RoutedEdge;
      quality: 'fast' | 'exact';
    }>
  | Readonly<{
      kind: 'degraded';
      route: RoutedEdge;
      quality: 'emergency';
      reason: RouteFailureReason;
    }>;
```

即使遇到极端非法布局：

```text
两个节点互相重叠
端口完全被另一个节点盖死
```

UI 也应该：

```text
显示一条 emergency 边
+
开发态可标记 degraded
```

而不是：

```text
什么都不画
```

---

## 2.2 不变量 B：显式 side 是硬约束

如果：

```ts
edge.target.side === 'left'
```

那么目标入口必须是：

```text
                         target
             approach     │
                ●─────────●
                          ↑
                       left anchor
```

计算：

```ts
anchor = {
  x: targetRect.x,
  y: targetRect.y + targetRect.height / 2,
};

approach = {
  x: anchor.x - ENDPOINT_CLEARANCE,
  y: anchor.y,
};
```

最后一段必须：

```text
approach -> anchor
```

并且只能水平向右进入 target。

不能因为另一侧路线更短就偷偷改成 top / right / bottom。

自动换边只允许发生在：

```ts
side === undefined
```

的自动模式。

---

## 2.3 不变量 C：端点必须先离开节点再转弯

保留现在正确的思想：

```text
source anchor
    │
    └── clearance ──► 才允许转弯
```

以及：

```text
先到 target approach
    │
    └── 最后一小段直线进入 target anchor
```

建议把它正式建模，不再只是 `offsetAnchor()` helper：

```ts
export type RoutingPort = Readonly<{
  nodeId: string;
  side: FlowAnchorSide;
  anchor: FlowPoint;
  escape: FlowPoint;
}>;
```

源端：

```text
anchor -> escape
```

目标端：

```text
escape -> anchor
```

路由主体只负责：

```text
source.escape -> target.escape
```

这样端点约束和主体寻路彻底分离。

---

# 3. 第一阶段：先把“线消失”今天就从架构上堵死

在上 V2 寻路之前，先做 P0。

---

## 3.1 增加 Route Fingerprint

新增：

```text
src/flow/routeFingerprint.ts
```

类型：

```ts
export type RouteFingerprint = Readonly<{
  sourceNodeId: string;
  sourceSide: FlowAnchorSide | null;
  targetNodeId: string;
  targetSide: FlowAnchorSide | null;

  sourceX: number;
  sourceY: number;
  sourceWidth: number;
  sourceHeight: number;

  targetX: number;
  targetY: number;
  targetWidth: number;
  targetHeight: number;
}>;
```

或者直接生成稳定 key：

```ts
function edgeRouteFingerprint(
  edge: FlowEdge,
  source: FlowNode,
  target: FlowNode,
): string
```

必须包含：

```text
source.nodeId
source.side
target.nodeId
target.side
source rect
target rect
```

所以：

```text
target.side: right -> left
```

会立刻让旧 exact route 失效。

不能等 Worker 回来才发现。

---

## 3.2 `useEdgeRoutes` 不再只检查 changedNodeIds

现在：

```text
node changed?
```

升级为：

```text
route fingerprint changed?
+
route 是否被移动障碍物扫到?
```

最小修复版本先做：

```ts
const currentFingerprint = edgeRouteFingerprint(...);

if (
  exact
  && exact.fingerprint === currentFingerprint
) {
  return exact;
}
```

这样即使节点没动，只改 target side，也会立刻执行新 preview。

---

## 3.3 Worker 返回失败时禁止覆盖掉 Last Known Good

当前：

```ts
setExactRoutes(
  new Map(event.data.routes.map(...))
);
```

Worker 如果某条 edge 没 route，它就从 map 消失。

改成显式 response：

```ts
type EdgeRouteResponse =
  | {
      edgeId: string;
      kind: 'routed';
      route: RoutedEdge;
    }
  | {
      edgeId: string;
      kind: 'failed';
      reason: RouteFailureReason;
    };
```

收到 `failed` 时：

```text
当前 preview 仍合法
    ↓
保留 preview

旧 exact 仍满足 fingerprint
    ↓
保留 old exact

两者都不合法
    ↓
生成 emergency route
```

绝不删除视觉边。

---

# 4. Move 热路径：不要“重新寻路”，先“修旧路”

这是性能的关键。

节点 move 是高度连续的：

```text
frame 1: x = 500
frame 2: x = 503
frame 3: x = 507
frame 4: x = 511
```

绝大多数情况下，上一帧的路由已经非常接近这一帧的答案。

所以每帧重新从零 A* 是错误模型。

---

## 4.1 Fast Repair

假设已有：

```text
S ─────────┐
           │
           └──────── T
```

只移动 source：

```text
S'
```

通常根本不需要重算整个路径。

只要尝试：

```text
S'.escape
   │
   └──── 接回旧路径第一个可见折点
```

目标没动时，旧路径后半段全部复用。

算法：

```text
1. 更新 source / target port
2. 保留 previous.points 的内部 bends
3. 从新 source.escape 尝试连接：
   - old point[1]
   - old point[2]
   - old point[3]
4. 从新 target.escape 反向做同样处理
5. 只碰撞检测新生成的少数 segment
6. 成功立即返回
```

普通拖动下这个命中率应该非常高。

目标：

```text
FastRepair hit rate > 90%
```

这样大部分帧根本不会进入 A*。

---

## 4.2 为什么 Fast Repair 比“每帧最短路”更适合编辑器

它同时解决三件事：

### 性能

只检查几个新 segment。

### 稳定性

不会节点移动 1px，线就突然从上绕切换到下绕。

### 视觉连续性

路径弯点移动更少，不会闪烁。

---

# 5. Dirty Edge：move 每帧只算真正受影响的边

只更新“与被拖节点相连的边”还不够。

因为一个没有连到该节点的 edge：

```text
A ─────────── B
```

可能被正在移动的节点 C 挡住：

```text
A ───── C ───── B
```

所以 dirty edge 应该来自两部分。

---

## 5.1 邻接边

建立：

```ts
Map<NodeId, ReadonlySet<EdgeId>>
```

移动 node C：

```text
dirty += adjacency[C]
```

复杂度接近：

```text
O(degree(C))
```

---

## 5.2 被移动障碍物扫到的已有路线

再建立：

```text
RouteSegmentSpatialIndex
```

每条 routed edge 拆成 segment：

```text
edge e1
  segment 0 rect
  segment 1 rect
  segment 2 rect
```

放入空间索引。

节点从：

```text
oldRect -> newRect
```

计算 swept rect：

```text
union(old inflated rect, new inflated rect)
+
routing padding
```

查询：

```ts
segmentIndex.query(sweptRect)
```

命中的 edge 全部标 dirty。

于是：

```text
dirtyEdges
=
incidentEdges(movedNodes)
∪
routesIntersectingSweptArea
```

这才是真正正确的动态避障失效集合。

---

# 6. 复用现有 SpatialHash，但不要每条 edge 临时重建

当前 `SpatialHash` 已经是一个很好的基础。

建议建立两个长期索引：

```text
ObstacleIndex
RouteSegmentIndex
```

---

## 6.1 ObstacleIndex

```ts
type ObstacleId = string;

type IndexedObstacle = Readonly<{
  nodeId: ObstacleId;
  rect: FlowRect;
}>;
```

节点拖动一帧时只更新：

```text
被移动节点
```

而不是：

```text
nodes.map(...)
```

全部重建。

---

## 6.2 RouteSegmentIndex

新增：

```text
src/flow/routeSegmentIndex.ts
```

内部记录：

```ts
type RouteSegmentRef = Readonly<{
  edgeId: string;
  segmentIndex: number;
}>;
```

某条 edge route 更新时：

```text
delete old segments
insert new segments
```

其他 edge 不动。

---

## 6.3 建议给 SpatialHash 加稳定 ID 层

当前 `SpatialHash<T>` 用对象 identity 做：

```text
set
delete
```

动态路由如果每次创建新的 segment object，会很容易管理混乱。

可以增加一个强类型 wrapper：

```ts
class SpatialIndexById<TId, TValue> {
  ...
}
```

内部：

```text
Map<TId, TValue>
+
SpatialHash<TValue>
```

调用层只用稳定：

```text
nodeId
edgeId:segmentIndex
```

不要让 route engine 依赖临时对象 identity。

---

# 7. 真正的寻路器：Sparse Orthogonal Visibility Graph

当 Fast Repair 失败时，不要回到全画布固定网格。

我建议换成：

# **稀疏正交可见图（Sparse Orthogonal Visibility Graph / OVG）**

这非常适合你现在的场景：

```text
全部障碍物都是 axis-aligned rectangle
全部连线希望是 orthogonal polyline
```

---

## 7.1 为什么它比 20px 网格更适合 Flow

正交最短路径真正有意义的转折位置主要来自：

```text
source / target ports
障碍物边界
障碍物 corner 外侧
```

不需要把整张空白画布切成：

```text
20px × 20px
```

的格子。

例如局部只有 8 个障碍物：

固定 grid A* 可能搜索：

```text
几千个 cell
```

Sparse OVG 只需要：

```text
几十个 vertex
```

---

## 7.2 Vertex 怎么生成

局部障碍物先膨胀：

```text
node rect + ROUTE_GAP
```

每个矩形生成 corner portal：

```text
(x1, y1)
(x2, y1)
(x1, y2)
(x2, y2)
```

这些点实际使用：

```text
已经膨胀后的 obstacle corner
```

再加入：

```text
source.escape
target.escape
```

必要时加入 source / target 对应的辅助投影点。

---

## 7.3 不要 O(M²) 全连接

对于每个 vertex，只连接：

```text
同一水平线上最近可见的 left / right
同一垂直线上最近可见的 top / bottom
```

做法：

```text
按 x 分桶 / 排序
按 y 分桶 / 排序
+
SpatialHash 做 segment collision query
```

形成稀疏图。

然后跑：

```text
A*
```

状态不是只有 point，而是：

```ts
type RouterState = {
  vertexId: number;
  incomingDirection: 'horizontal' | 'vertical' | null;
};
```

这样可以直接加入转弯成本。

---

## 7.4 Cost

建议：

```text
cost
=
Manhattan length
+
bendPenalty
+
routeChangePenalty
+
optional congestionPenalty
```

第一版：

```text
bendPenalty = 16~24
routeChangePenalty = 轻微
```

目的不是数学意义绝对最短，而是：

```text
短
+
少拐弯
+
拖动时别乱跳
```

---

# 8. Local Corridor：寻路只看旧路径附近

即使 OVG 比 grid 快，也不要默认把全画布所有障碍塞进去。

拖动时已有 previous route。

以旧 route bounds 为中心建立 corridor：

```text
previous route bounds
∪
source rect
∪
target rect
```

向外扩：

```text
96px
```

只查询这个区域的 obstacle。

如果找不到：

```text
96
 ↓
192
 ↓
384
 ↓
global
```

分级扩张。

这样常见情况：

```text
1000 个节点
```

但局部 corridor 可能只有：

```text
7~20 个障碍物
```

路由开销就由：

```text
N = 1000
```

变成：

```text
M = nearby obstacles
```

---

# 9. target = left 的正确处理流程

用户明确把 target 改成左侧时，不要让“寻路器自己猜”。

流程应该是：

```text
target.side = left
        │
        ▼
buildTargetPort()
        │
        ├─ anchor   = target 左边中点
        └─ escape   = anchor 向左 clearance
        │
        ▼
core router 只负责：
source.escape -> target.escape
        │
        ▼
append:
target.escape -> target.anchor
```

完整：

```text
source anchor
    │
source escape
    │
    └─────────┐
              │
         自动避开障碍
              │
    ┌─────────┘
target escape
    │
    └────────────► target left anchor
```

如果局部路径失败：

```text
扩大 corridor
```

而不是：

```text
return null
```

---

# 10. 源/目标节点本身不要再靠“特殊 if”勉强规避

当前实现同时存在：

```text
excluded source / target
+
endpointRects
+
offsetAnchor
+
joinEndpointSegments
```

逻辑分散。

V2 建议统一为：

```text
普通障碍物：
    inflated rect，禁止进入

source / target：
    actual/inflated body + 一个合法 port tunnel
```

可以理解为目标左侧开一扇门：

```text
┌────────────────────┐
│                    │
│ target        ◄────┼── only legal tunnel
│                    │
└────────────────────┘
```

主体 path 不能穿 target body。

只有最后：

```text
escape -> anchor
```

这一段允许进入。

端口规则从算法结构上成立，就不容易继续出现：

```text
左侧锚点被 endpoint collision checker 自己封死
```

这种问题。

---

# 11. Move 生命周期建议

当前节点 drag 已经用 `requestAnimationFrame` 合帧，这一点保留。

但路由器需要知道：

```text
现在是 drag 中
还是 drag 已结束
```

建议在通用 Flow state 增加强类型瞬时交互状态：

```ts
export type RoutingInteraction =
  | Readonly<{
      kind: 'idle';
    }>
  | Readonly<{
      kind: 'node-drag';
      nodeIds: ReadonlySet<string>;
      interactionId: number;
    }>;
```

如果不想把 `Set` 放入状态，也可以：

```ts
nodeIds: ReadonlyArray<string>
```

---

## 11.1 drag start

```text
记录 old obstacle rect
标记 interaction = node-drag
暂停 exact worker 连续提交
```

---

## 11.2 每个 RAF

```text
1. 应用节点位置
2. 更新 moved node 的 ObstacleIndex
3. 计算 swept rect
4. 查询 dirty edge ids
5. FastRepair
6. repair 失败 -> local OVG
7. 更新 RouteCache
8. 更新 RouteSegmentIndex
9. render
```

禁止：

```text
route all edges
```

---

## 11.3 drag end

```text
flush 最后一帧
interaction = idle
收集本次 dirty edges
一次性发给 worker 做 exact settle
```

Worker 回来后只替换：

```text
对应 dirty edge
```

其他 route cache 不动。

---

# 12. Worker 协议：第一版不用过度设计

当前每帧都把全量：

```text
nodes + edges
```

postMessage 给 Worker 不值。

第一版最简单的高收益改法：

## drag 中

```text
不发 exact worker
```

主线程使用：

```text
FastRepair + Local OVG
```

已经足够稳定。

## drag end

只发一次：

```ts
type ExactRouteRequest = Readonly<{
  revision: number;
  nodes: ReadonlyArray<FlowNode>;
  edges: ReadonlyArray<FlowEdge>;
  dirtyEdgeIds: ReadonlyArray<string>;
}>;
```

Worker：

```text
建一次 obstacle index
只 route dirtyEdgeIds
返回 patch
```

而不是返回整张 route map。

后续如果 1000+ node 场景仍然希望进一步压低 structured clone，再升级成 persistent worker model：

```text
init snapshot
+
node patches
+
edge patches
+
dirty route requests
```

但这个放 P3，不要第一步就把协议复杂化。

---

# 13. Route Cache

新增：

```text
src/flow/routeCache.ts
```

建议状态：

```ts
export type CachedRoute = Readonly<{
  edgeId: string;
  fingerprint: string;
  route: RoutedEdge;
  quality: RouteQuality;
  obstacleRevision: number;
}>;
```

不要把：

```text
React state
```

本身当成 route engine。

`useEdgeRoutes.ts` 应该只负责：

```text
订阅 nodes / edges / interaction
调用 route engine
把结果暴露给 React
管理 Worker 生命周期
```

几何、失效、修补、寻路全部放纯模块。

这也符合仓库现在“高内聚、入口只编排”的规范。

---

# 14. 推荐拆文件

不要继续把所有逻辑塞进 `routing.ts`。

建议：

```text
src/flow/
├─ routing.ts
│   └─ 对外 facade / 组合
│
├─ routingPort.ts
│   ├─ buildSourcePort
│   ├─ buildTargetPort
│   └─ port constraint
│
├─ routeFingerprint.ts
│
├─ routeCache.ts
│
├─ routeInvalidation.ts
│   ├─ adjacency
│   ├─ swept bounds
│   └─ dirty edge set
│
├─ routeSegmentIndex.ts
│
├─ routeRepair.ts
│   └─ fast repair
│
├─ orthogonalVisibilityGraph.ts
│   ├─ local obstacle collection
│   ├─ vertex generation
│   ├─ visibility neighbors
│   └─ A*
│
├─ routing.worker.ts
│
└─ useEdgeRoutes.ts
```

原来的：

```text
routingPathfinder.ts
```

可以先保留作为 P0 emergency / 回归对照。

OVG 稳定后再删，不需要在第一轮同时重构所有东西。

---

# 15. 高性能数据流

最终 move 一帧应当长这样：

```text
PointerEvent
   │
   ▼
RAF coalescer
   │
   ▼
moveSelectedNodes
   │
   ├──────────────► Node render
   │
   ▼
Router.updateMovedNodes
   │
   ├─ update obstacle index: O(moved nodes)
   │
   ├─ incident edge lookup: O(degree)
   │
   ├─ swept area route query: O(local)
   │
   ▼
dirtyEdgeIds
   │
   ▼
for dirty edge
   │
   ├─ FastRepair
   │      └─ success ─────────────┐
   │                              │
   └─ Local Sparse OVG            │
          └─ route ───────────────┤
                                  ▼
                         incremental route cache
                                  │
                         incremental segment index
                                  │
                                  ▼
                             SVG update
```

注意：

```text
没有全 edges loop
没有全 nodes loop / dirty edge
没有每帧 Worker
没有全画布 grid
```

---

# 16. 复杂度目标

设：

```text
N = 全部节点
E = 全部边
D = 当前 dirty edges
M = dirty route corridor 内附近障碍物
B = 一条旧 route 的 bend 数
```

当前 move 最差方向接近：

```text
O(D * N)
+
grid search
+
后台 O(E * routeCost)
```

V2 正常热路径：

```text
dirty discovery:
O(degree + local segment hits)

FastRepair:
O(D * B * local collision query)

local OVG fallback:
O(M log M + sparse A*)
```

关键是：

```text
性能跟局部 M / D 走
```

而不是：

```text
跟全局 N / E / 画布面积走
```

---

# 17. 建议性能指标

不要只靠“感觉不卡”。

开发态加 RouterStats：

```ts
export type RouterStats = Readonly<{
  dirtyEdgeCount: number;
  nearbyObstacleCount: number;
  fastRepairHits: number;
  localGraphVertices: number;
  expandedStates: number;
  routeTimeMs: number;
}>;
```

只在 dev/debug 使用。

建议目标：

| 场景 | 目标 |
|---|---:|
| 500 nodes / 700 edges，拖动普通节点 | main-thread routing p95 < 1.5ms/frame |
| 同场景 p99 | < 3ms/frame |
| 普通拖动 FastRepair 命中率 | > 90% |
| drag 过程中 Worker 全图请求 | 0 |
| drag end dirty exact settle | < 30ms（典型少量 dirty edge） |
| target side 修改 | 下一帧立即生效 |
| router failure | 0 条 edge 消失 |

这里真正关键的不是追求每次 0.1ms，而是：

```text
稳定在一帧预算内
+
没有长尾
+
没有全图重算
```

---

# 18. 测试必须补，不然这次还会回归

当前 `routing.test.ts` 已经覆盖：

```text
绕过 obstacle
move endpoint
endpoint clearance
backward connection
```

但是正好没有覆盖这次截图里的核心情况。

---

## 18.1 target side 改变，但节点完全不动

新增 `useEdgeRoutes.test.tsx`：

```text
given:
    edge target.side = right
    exact route 已存在

when:
    nodes 引用/geometry 不发生变化
    edge target.side -> left

then:
    下一次 render 立刻得到 left target preview
    不能继续复用旧 right-side exact
```

这是 P0 必须有的回归。

---

## 18.2 locked target left + 中间障碍物

```text
source                      target
  ●        blocker       ◄──●
  └─────── █████ ──────────┘
```

断言：

```text
route != missing
route.targetSide === 'left'
最后一段水平进入 target
全部主体 segment 不穿 obstacle
```

---

## 18.3 unrelated moved node 挡住已有 edge

初始：

```text
A ───────────── B

          C
```

移动 C：

```text
A ───── C ───── B
```

C 并不属于 A-B edge。

断言：

```text
A-B 被 dirty invalidation 命中
下一帧 route 绕开 C
```

---

## 18.4 Worker 某条 edge exact 失败，UI 不得删除旧 route

模拟：

```text
worker returns:
edge-1 = routed
edge-2 = failed
```

断言：

```text
edge-2 仍存在可渲染 route
```

---

## 18.5 pathological overlap

两个节点重叠到端口没有自由空间。

断言不是：

```text
route == null
```

而是：

```text
quality === 'emergency'
```

同时 route 仍可被 SVG 渲染。

---

# 19. 不建议的方案

---

## 19.1 不建议继续调 `GRID_SIZE`

比如：

```text
20 -> 30
```

确实会少搜索。

但代价是：

```text
路径更粗糙
狭窄通道更容易找不到
端点接入更怪
```

本质问题没变。

---

## 19.2 不建议 move 每帧把 A* 丢 Worker

Worker 不阻塞主线程不代表免费。

仍然有：

```text
structured clone
任务调度
旧结果失效
route replacement
CPU 消耗
```

拖动这种高频连续状态，正确做法是：

```text
主线程廉价增量 preview
+
交互结束一次精修
```

---

## 19.3 不建议只重算 moved node 的 incident edges

这样性能快，但 correctness 不完整。

正在拖的节点本身是障碍物。

它可以挡住完全不相连的 edge。

必须同时有：

```text
RouteSegmentSpatialIndex
```

---

## 19.4 不建议只保留旧 route 不做碰撞检查

这样不会消失，但会直接穿节点。

旧 route 只能作为：

```text
repair seed
```

不能作为无条件答案。

---

# 20. P0 / P1 / P2 / P3 实施顺序

## P0：先修正确性和“不消失”

修改：

```text
route fingerprint
edge side dirty invalidation
worker failed result
always-visible render contract
target-left regression tests
```

这一阶段即使仍用旧 A*，用户现在这个 bug 也必须先关闭。

验收：

```text
target right -> left
线立即换入口
不会消失
```

---

## P1：增量 dirty edge

加入：

```text
EdgeAdjacency
ObstacleIndex
RouteSegmentIndex
swept rect invalidation
```

move 不再全边参与。

验收：

```text
拖 1 个节点
只看到少量 dirty edges
```

开发态 stats 可以直接打印计数验证。

---

## P2：Fast Repair + Local Sparse OVG

加入：

```text
previous route repair
local corridor obstacle query
sparse visibility graph
direction-aware A*
```

旧 fixed-grid A* 退到 emergency。

验收：

```text
普通拖动几乎都走 FastRepair
复杂绕障才走 local OVG
```

---

## P3：Worker exact settle

Worker 改成：

```text
drag 中不跑
drag end 跑 dirty batch
```

如果未来 1000+ node 后仍有 clone 压力，再做 persistent worker patch protocol。

---

# 21. 最小接口草案

推荐最终对外门面保持简单：

```ts
export type RouteEngineInput = Readonly<{
  nodes: ReadonlyArray<FlowNode>;
  edges: ReadonlyArray<FlowEdge>;
  interaction: RoutingInteraction;
}>;

export type RouteEngineOutput = Readonly<{
  routes: ReadonlyArray<RoutedEdge>;
  dirtyEdgeIds: ReadonlySet<string>;
}>;

export interface FlowRouteEngine {
  update(input: RouteEngineInput): RouteEngineOutput;
  applyExactRoutes(
    revision: number,
    routes: ReadonlyArray<RoutedEdge>,
  ): void;
}
```

React Hook：

```ts
useEdgeRoutes(...)
```

只做：

```text
读状态
routeEngine.update
Worker 编排
触发 render
```

不要再让 Hook 自己承担：

```text
碰撞策略
路径算法
dirty graph 算法
fallback 决策
```

---

# 22. 一条 edge 的完整决策树

最终每条 dirty edge：

```text
                       edge dirty
                           │
                           ▼
                  fingerprint changed?
                   /               \
                 yes               no
                 │                  │
         rebuild endpoint ports     │
                 └──────────┬───────┘
                            ▼
                    previous route?
                      /          \
                    yes          no
                    │             │
                    ▼             │
                 FastRepair       │
                 /       \        │
              success    fail     │
                │          └──┬────┘
                │             ▼
                │       Local Sparse OVG
                │          /       \
                │       success    fail
                │          │         │
                │          │     expand corridor
                │          │         │
                │          │      global OVG
                │          │         │
                └──────┬───┴─────────┘
                       ▼
                   RouteCache
                       │
                       ▼
                    render
```

如果连 global OVG 都因为非法重叠状态失败：

```text
emergency visible route
+
diagnostic
```

仍然不能：

```text
return null -> disappear
```

---

# 23. 对截图问题的最终预期行为

你把：

```text
“写入运行时搜索词”
```

这条边的 target 改成该节点左侧后：

### 当前

```text
side changed
  ↓
旧 exact cache 可能仍被认为有效
  ↓
worker / preview 路由失败
  ↓
null
  ↓
edge 消失
```

### V2

```text
target.side right -> left
  ↓
fingerprint 立即变化
  ↓
旧 exact 不再可直接复用
  ↓
建立 left RoutingPort
  ↓
FastRepair 尝试把旧主体接到新 target.escape
  ↓
被节点挡住？
  ├─ no  -> 立即显示
  └─ yes -> Local Sparse OVG 自动绕行
                  ↓
              立即显示
                  ↓
      idle 后 Worker 精修 dirty edge
```

用户看到的应该是：

> **线在下一帧重新绕到左侧入口，而不是消失。**

---

# 24. 最终建议

这一轮不要再把目标定成：

> “修一下 A*，让这个 case 能过。”

应该把目标定成：

> **把路由系统从“无状态全量寻路器”升级成“有缓存、有失效图、有动态修补能力的交互式增量路由引擎”。**

因为真正的 Flow 编辑器和一次性 pathfinding 完全不是同一个问题。

你现在已经有：

```text
自研 Flow
SpatialHash
RAF pointer coalescing
Worker backpressure
正交 path
endpoint clearance
```

这些基础其实够了。

缺的是：

```text
Route identity
Dirty propagation
Persistent spatial indices
Fast repair
Sparse local exact routing
Failure contract
```

把这六块补上以后，连线这部分才会从“能画”进入“编辑器级别”。

---

# 25. 建议最终验收清单

- [ ] `target.side` 从任意方向改到 `left`，下一帧立即切换入口。
- [ ] 显式 source/target side 永远作为硬约束。
- [ ] 任意路由失败都不会让 edge 从 UI 消失。
- [ ] 普通拖动过程中不发送全图 Worker exact request。
- [ ] move 一帧只重算 dirty edges。
- [ ] moved node 能使与它不相连、但被它挡住的 edge 自动 reroute。
- [ ] ObstacleIndex 在 drag 时只增量更新 moved nodes。
- [ ] RouteSegmentIndex 在 route 变化时只增量更新对应 edge。
- [ ] FastRepair 命中时不启动图搜索。
- [ ] FastRepair 失败时只构造 local corridor OVG。
- [ ] local OVG 失败后按 96/192/384/... 扩大 corridor。
- [ ] grid A* 不再作为 move 主算法。
- [ ] Worker drag-end 只精修 dirty edges。
- [ ] 增加 target-side-change 回归测试。
- [ ] 增加 unrelated-obstacle-crossing 回归测试。
- [ ] 增加 worker route failure 保持可见回归测试。
- [ ] 增加 500 nodes / 700 edges 的开发态性能场景。
- [ ] 记录 dirtyEdgeCount / fastRepairHitRate / routeTimeMs 等指标。
- [ ] `src/flow` 保持纯画布内核，不依赖 `features`。
- [ ] `useEdgeRoutes.ts` 只保留编排职责，算法拆入具名纯模块。

---

## 一句话版

**端点换边立即失效旧缓存；edge 永不因为 router failure 消失；move 用“空间索引找 dirty edge + 旧路径 Fast Repair”，复杂情况再走局部 Sparse OVG，拖动结束后 Worker 只精修 dirty edges。**
