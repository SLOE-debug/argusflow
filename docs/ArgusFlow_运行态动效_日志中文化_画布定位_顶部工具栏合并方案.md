# ArgusFlow 编辑器运行态动效、日志可读性、画布定位与顶部工具栏合并方案

> 仓库：`SLOE-debug/argusflow`  
> 分析分支：`main`  
> 分析基线：`d7602b11551f34ebf4afd7d0ccfc5437064b55e8`（2026-08-26）  
> 目标：解决运行时节点/连线反馈弱、画布“定位/适应内容”无效、执行日志难读、编辑命令栏与系统标题栏重复占用纵向空间的问题。  
> 原则：**不破坏现有 Workflow Runtime 契约，不把展示文案继续塞进底层执行协议；优先在前端运行态层和 Flow 内核补齐。**

---

## 0. 结论先行

这次我建议不要“零散补几个 CSS 动画”，而是把编辑器里的**运行态展示**正式当成一层：

```text
Rust Runtime
    │
    │ ExecutionEvent
    ▼
useWorkflowStudio
    │
    ├── Runtime presentation state
    │     ├── node state
    │     ├── edge state
    │     └── run state
    │
    ├── ExecutionLog view model
    │     ├── 中文事件名
    │     ├── 节点显示名
    │     ├── 同节点稳定颜色
    │     └── 用户日志 / 技术日志分层
    │
    ▼
FlowCanvas / NodeCard / FlowEdges
```

本次四个目标建议一起做：

1. **节点运行状态**
   - 待运行：灰色/弱蓝色
   - 正在运行：蓝色呼吸边框 + 状态图标
   - 已完成：绿色勾
   - 失败：红色
   - 分支未走到：运行结束后标记“未执行”，避免永远停留在“待运行”

2. **连线运行状态**
   - 当前经过的边显示“电流脉冲”
   - 不再只画几个一次性圆点
   - 使用 SVG `stroke-dasharray + stroke-dashoffset` 做连续脉冲
   - `edge_traversed` 事件触发 700~900ms 的流动动画

3. **正常的“定位 / 适应内容”**
   - “定位”：优先定位选中节点；没有选中节点时居中全部内容，保持当前缩放
   - “适应内容”：计算所有节点包围盒，自动缩放并居中
   - 不再两个按钮都执行 `setViewport({ x: 0, y: 42, zoom: 1 })`

4. **顶部工具栏合并**
   - 去掉独立的第二行 `EditorCommandBar`
   - 编辑命令 + 校验 + 运行 + 发布全部并入 `WindowTitleBar`
   - 编辑器从四行布局变为三行布局：

```text
┌──────────────────────────────────────────────────────────────┐
│ TitleBar + 编辑命令 + 校验 / 运行 / 发布 + Search + Window  │
├──────────────────────────────────────────────────────────────┤
│ 节点库 │                  画布                   │ 属性     │
│        │                                         │          │
│        ├─────────────────────────────────────────┤          │
│        │                底部运行面板              │          │
├──────────────────────────────────────────────────────────────┤
│ StatusBar                                                    │
└──────────────────────────────────────────────────────────────┘
```

---

# 1. 当前代码现状

现在仓库其实已经具备一部分基础能力，只是没有收口成完整体验。

## 1.1 节点已经有运行状态，但状态粒度不够

当前：

```ts
export type NodeRunState =
  | 'idle'
  | 'running'
  | 'success'
  | 'error';
```

`applyExecutionEventToNodes()` 已经会处理：

```text
node_started   -> running
node_succeeded -> success
node_failed    -> error
```

`WorkflowNodeCard.tsx` 也已经有：

```ts
const STATUS_TONES = {
  idle: 'bg-slate-400',
  running: 'animate-pulse bg-blue-500',
  success: 'bg-emerald-500',
  error: 'bg-rose-500',
};
```

问题是现在 UI 只在卡片右边放一个小圆点。

对于 RPA / Workflow IDE 来说，这个反馈太弱。

用户真正需要的是一眼知道：

```text
还没执行
    ↓
正在执行
    ↓
执行成功
```

而不是找卡片角落里的 6px 小点。

---

## 1.2 连线已经有 activeEdge，但现在更像“粒子经过”

现在 `FlowEdges.tsx` 已经有：

```ts
const ACTIVE_PARTICLES = [0, 1, 2, 3] as const;
```

并通过：

```tsx
<animateMotion
  dur="900ms"
  path={path}
  repeatCount="1"
/>
```

让 4 个圆点沿路径移动。

同时 `useWorkflowStudio.ts` 已经监听：

```ts
if (payload.kind === 'edge_traversed' && payload.edge_id) {
  state.activateEdge(payload.edge_id);
}
```

`createFlowStore.tsx` 里也已经有：

```ts
activateEdge(edgeId, duration = 900)
```

所以**事件链路已经是通的**。

真正要改的是视觉方案：

```text
当前：
──────●──●──●──────>

建议：
────▰▰────▰▰────▰▰──>
     电流沿线滚动
```

更适合用 SVG path 的虚线偏移动画，而不是小球。

---

## 1.3 “居中画布”和“适应内容”现在确实是同一个假实现

当前 `FlowCanvasTools.tsx`：

```ts
const resetViewport = () =>
  setViewport({ x: 0, y: 42, zoom: 1 });
```

然后：

```tsx
<ToolButton label="居中画布" onClick={resetViewport}>
  <Crosshair />
</ToolButton>

<ToolButton label="适应内容" onClick={resetViewport}>
  <Maximize2 />
</ToolButton>
```

所以截图里你说“现在点了瞎定位”是准确的：

**这两个按钮根本没有根据节点位置计算 viewport。**

这块应该补成 Flow 内核能力，而不是 Workflow 业务组件自己算。

---

## 1.4 日志底层事件已经结构化，但 UI 把内部枚举直接暴露出来了

当前 `ExecutionLog.tsx` 直接显示：

```text
workflow_started
node_started
backend_selected
node_succeeded
edge_traversed
workflow_completed
```

代码是：

```tsx
<span>
  {event.kind}
</span>
```

这和 AQL 那块之前的问题一样：

> **协议内部 vocabulary 直接漏到了产品 UI。**

后端其实已经有不少中文消息，例如：

```text
开始执行工作流：xxx
工作流执行完成
应用会话已获取
命令执行完成，退出代码 0
已产生 1 个值输出
自动化失败现场已保存
```

但 `node_label()` 仍然是：

```text
Start
Log
Debug Output
Delay 500ms
Condition
Application
UI Click
UI SetValue
Command ...
End
```

所以目前日志会天然出现：

```text
node_started [xxx] UI Click
backend_selected [xxx] UIA
node_succeeded [xxx]
```

用户读起来像是在看 protocol trace，而不是工作流运行记录。

---

## 1.5 编辑器纵向空间被 TitleBar + CommandBar 重复占了 80px

当前 `App.tsx` 编辑器布局：

```text
40px WindowTitleBar
40px EditorCommandBar
1fr  Main
40px StatusBar
```

对应：

```ts
grid-rows-[40px_40px_minmax(0,1fr)_40px]
```

而真正的命令：

```text
Undo
Redo
Copy
Paste
Duplicate
Delete
Library
Console
Inspector

Validate
Run
Publish
```

全部在 `EditorCommandBar.tsx`。

你截图红框标出来的位置正好适合把这一整行收进系统 TitleBar。

---

# 2. 推荐的运行态状态机

我建议把：

```ts
'idle' | 'running' | 'success' | 'error'
```

升级为：

```ts
export type NodeRunState =
  | 'idle'
  | 'pending'
  | 'running'
  | 'success'
  | 'error'
  | 'skipped';
```

语义：

| 状态 | 中文 | 什么时候出现 |
|---|---|---|
| `idle` | 未运行 | 编辑状态 / 尚未开始 |
| `pending` | 待运行 | 点击运行、校验通过后 |
| `running` | 正在运行 | 收到 `node_started` |
| `success` | 已完成 | 收到 `node_succeeded` |
| `error` | 失败 | 收到 `node_failed` |
| `skipped` | 未执行 | 流程结束时仍为 pending 的分支节点 |

这里一定要有 `skipped`。

因为 Condition 分支执行时：

```text
             ┌── A ──┐
Condition ───┤       ├── End
             └── B ──┘
```

一次运行只会走 A 或 B。

如果只有：

```text
pending
running
success
```

那没走的 B 在流程结束后仍然是：

```text
待运行
```

这会让用户误以为流程没结束。

所以应该：

```text
workflow_completed / workflow_failed
    ↓
剩余 pending
    ↓
skipped
```

中文显示：

```text
未执行
```

---

# 3. 节点动效方案

## 3.1 不要只改状态点，整个节点卡片都要参与反馈

建议：

### idle

```text
普通卡片
无运行状态 icon
```

### pending

```text
┌─────────────────┐
│ ○ 读取搜索结果   │
│   等待执行       │
└─────────────────┘
```

视觉：

```text
border-slate-200
bg-white
右上/右侧：Clock3 或空心圆
```

不要动画。

---

### running

```text
╔═════════════════╗
║ ◉ 查找按钮       ║
║   正在运行       ║
╚═════════════════╝
```

建议三个层次同时存在：

1. 蓝色边框
2. 很弱的 glow
3. 外圈呼吸动画

例如：

```ts
running:
  'border-blue-500 ' +
  'shadow-[0_0_0_3px_rgba(59,130,246,.10),0_6px_18px_rgba(37,99,235,.14)] ' +
  'after:absolute after:-inset-[3px] after:rounded-[10px] ' +
  'after:border after:border-blue-400/50 after:animate-[node-running-ring_1.4s_ease-in-out_infinite]'
```

建议不要让整张卡片上下跳动。

RPA 执行过程中画布应该稳定，动画只表示状态，不应该干扰位置感。

---

### success

```text
┌─────────────────┐
│ ✓ 查找按钮       │
│   已完成         │
└─────────────────┘
```

建议：

```text
border-emerald-300
右侧 CircleCheck
完成瞬间 150~220ms 轻微 scale / flash
之后完全静止
```

不要一直绿光闪。

---

### error

```text
┌─────────────────┐
│ ! 查找按钮       │
│   执行失败       │
└─────────────────┘
```

建议：

```text
border-rose-400
bg-rose-50/30
右侧 CircleX
```

可有一次性 `shake`，但我更建议**不要 shake**。

工作流编辑器不是消费 App，错误节点保持稳定更利于排查。

---

## 3.2 状态信息不要覆盖节点类型颜色

当前节点左边已经有：

```text
日志        blue
Debug       fuchsia
Delay       amber
Condition   violet
Application indigo
UI          cyan
Command     slate
```

这个颜色代表的是：

```text
节点类型
```

运行态颜色代表：

```text
生命周期
```

两者不要混成一个维度。

建议：

```text
左侧 4px accent
    = 节点类型

卡片边框 / 外环 / 状态 icon
    = 运行状态
```

这样用户不会因为“成功”导致所有节点都失去原有类型辨识度。

---

## 3.3 推荐修改 `WorkflowNodeCard.tsx`

建议抽出：

```ts
type RuntimeTone = Readonly<{
  surface: string;
  status: string;
  label: string;
}>;
```

例如：

```ts
const RUN_STATE_TONES: Record<NodeRunState, RuntimeTone> = {
  idle: {
    surface: '',
    status: '',
    label: '',
  },
  pending: {
    surface: 'border-slate-300',
    status: 'text-slate-400',
    label: '待运行',
  },
  running: {
    surface:
      'border-blue-500 shadow-[0_0_0_3px_rgba(59,130,246,.10)]',
    status: 'text-blue-600',
    label: '正在运行',
  },
  success: {
    surface: 'border-emerald-300',
    status: 'text-emerald-600',
    label: '已完成',
  },
  error: {
    surface: 'border-rose-400 bg-rose-50/30',
    status: 'text-rose-600',
    label: '失败',
  },
  skipped: {
    surface: 'opacity-55',
    status: 'text-slate-400',
    label: '未执行',
  },
};
```

注意：

**selected 的蓝色选中态不能完全覆盖 running。**

建议优先级：

```text
error
  >
running
  >
selected
  >
success / pending / idle
```

至少保证正在运行节点即使没有被选中，也非常明显。

---

# 4. 点击运行后，先把所有节点切成“待运行”

当前 `run()` 一开始做的是：

```ts
data: {
  ...node.data,
  runState: 'idle',
  invalid: false,
}
```

建议改成两阶段。

## 4.1 点击运行

先清掉旧结果：

```ts
runState: 'idle'
```

完成 validate。

## 4.2 validate 通过，真正开始运行前

统一：

```ts
runState: 'pending'
```

例如：

```ts
const markNodesPending = () => {
  const state = flowStore.getState();

  state.setNodes(
    state.nodes.map((node) => ({
      ...node,
      data: {
        ...node.data,
        runState: 'pending',
        invalid: false,
      },
    })),
    false,
  );
};
```

然后：

```ts
setRunning(true);
await runWorkflow(...);
```

这样用户按下“运行”以后，画布马上反馈：

```text
所有节点进入待运行
        ↓
Start 正在运行
        ↓
Start 已完成
        ↓
下一节点正在运行
```

不会出现“按钮已经运行中，但画布完全没变化”的空窗。

---

# 5. `applyExecutionEventToNodes` 完整状态转换

建议改为：

```ts
export function applyExecutionEventToNodes(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  event: ExecutionEvent,
): WorkflowCanvasNode[] {
  if (event.kind === 'workflow_started') {
    return nodes.map((node) => ({
      ...node,
      data: {
        ...node.data,
        runState:
          node.data.runState === 'idle'
            ? 'pending'
            : node.data.runState,
        invalid: false,
      },
    }));
  }

  if (
    event.kind === 'workflow_completed'
    || event.kind === 'workflow_failed'
  ) {
    return nodes.map((node) => ({
      ...node,
      data: {
        ...node.data,
        runState:
          node.data.runState === 'pending'
            ? 'skipped'
            : node.data.runState,
      },
    }));
  }

  const runState =
    event.kind === 'node_started'
      ? 'running'
      : event.kind === 'node_succeeded'
        ? 'success'
        : event.kind === 'node_failed'
          ? 'error'
          : null;

  if (!event.node_id || !runState) {
    return [...nodes];
  }

  return nodes.map((node) =>
    node.id === event.node_id
      ? {
          ...node,
          data: {
            ...node.data,
            runState,
          },
        }
      : node,
  );
}
```

---

# 6. 连线动效：改成“电流脉冲”，保留现有事件模型

这里我不建议改 Rust Runtime。

当前：

```text
NodeSucceeded
    ↓
EdgeTraversed(edge_id)
    ↓
NodeStarted(next)
```

这个时序已经足够表达：

```text
数据 / 控制权沿哪条边移动
```

继续用：

```text
edge_traversed
```

即可。

---

## 6.1 删除圆点式 ActiveEdgeParticles

现在：

```tsx
<ActiveEdgeParticles path={route.path} />
```

建议替换为：

```tsx
<ActiveEdgePulse path={route.path} />
```

---

## 6.2 推荐 SVG 结构

每条 edge：

```text
base path
selected / hovered overlay
runtime pulse overlay
arrow
```

运行脉冲用一个额外 path：

```tsx
function ActiveEdgePulse({
  path,
}: Readonly<{ path: string }>) {
  return (
    <path
      d={path}
      fill="none"
      stroke="#3b82f6"
      strokeLinecap="round"
      strokeWidth="3.2"
      strokeDasharray="2 11"
      vectorEffect="non-scaling-stroke"
      className="
        pointer-events-none
        motion-reduce:hidden
        animate-[flow-current_650ms_linear_infinite]
      "
      style={{
        filter:
          'drop-shadow(0 0 3px rgba(59,130,246,.95)) ' +
          'drop-shadow(0 0 7px rgba(96,165,250,.5))',
      }}
    />
  );
}
```

CSS：

```css
@keyframes flow-current {
  to {
    stroke-dashoffset: -26;
  }
}
```

效果：

```text
━━━━━━━━━━━━━━━━━━
  █     █     █
     █     █     █   →
```

比圆点更像真正的“电线脉冲”。

---

## 6.3 active 时间建议

当前：

```ts
DEFAULT_EDGE_ACTIVATION_MS = 900;
```

可以继续保留 900ms。

我更推荐：

```text
700~900ms
```

原因：

- 太短看不到
- 太长下一节点都已经跑起来，上一条线还亮着会显得滞后
- 900ms 对 UI 自动化节点比较适合

---

## 6.4 快节点连续运行时不要互相覆盖

例如：

```text
Start -> Log -> Delay
```

Start / Log 可能几十毫秒就走完。

所以：

```ts
activeEdgeIds: Record<string, expires>
```

这个设计是正确的。

不要改成：

```ts
activeEdgeId: string | null
```

否则快速事件会导致前一个 edge 动画瞬间消失。

---

## 6.5 建议增加 edge 的完成痕迹

不是必须，但效果会更完整。

增加：

```ts
traversedEdgeIds
```

运行期间：

```text
未经过：
#94a3b8

正在经过：
蓝色电流动画

已经经过：
#60a5fa / #93c5fd
```

这样流程跑到中间时：

```text
✓ Start ═════ ✓ A ═════ ◉ B ───── ○ C
          已走过            正在      待运行
```

会非常直观。

如果想控制改动量，这个可以放第二阶段。

---

# 7. “定位”与“适应内容”应该进入 Flow 内核

这块建议不要在 `WorkflowCanvas` 写。

因为它属于：

```text
Flow viewport capability
```

后面任何 Flow 都会用。

建议新增：

```text
src/flow/viewport.ts
```

---

# 8. 先实现节点包围盒

```ts
import type {
  FlowNode,
  FlowRect,
} from './types';

export function getNodesBounds(
  nodes: ReadonlyArray<FlowNode>,
): FlowRect | null {
  if (nodes.length === 0) return null;

  const minX = Math.min(
    ...nodes.map((node) => node.position.x),
  );

  const minY = Math.min(
    ...nodes.map((node) => node.position.y),
  );

  const maxX = Math.max(
    ...nodes.map(
      (node) => node.position.x + node.size.width,
    ),
  );

  const maxY = Math.max(
    ...nodes.map(
      (node) => node.position.y + node.size.height,
    ),
  );

  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}
```

---

# 9. 实现“适应内容”

输入：

```text
content bounds
canvas width
canvas height
padding
maxZoom
```

输出：

```text
ViewportTransform
```

建议：

```ts
export function fitBoundsToViewport(
  bounds: FlowRect,
  size: Readonly<{
    width: number;
    height: number;
  }>,
  options?: Readonly<{
    padding?: number;
    minZoom?: number;
    maxZoom?: number;
  }>,
): ViewportTransform {
  const padding = options?.padding ?? 64;
  const minZoom = options?.minZoom ?? 0.15;
  const maxZoom = options?.maxZoom ?? 2.5;

  const availableWidth =
    Math.max(1, size.width - padding * 2);

  const availableHeight =
    Math.max(1, size.height - padding * 2);

  const contentWidth =
    Math.max(bounds.width, 1);

  const contentHeight =
    Math.max(bounds.height, 1);

  const zoom = clamp(
    Math.min(
      availableWidth / contentWidth,
      availableHeight / contentHeight,
    ),
    minZoom,
    maxZoom,
  );

  return {
    zoom,
    x:
      size.width / 2
      - (bounds.x + bounds.width / 2) * zoom,
    y:
      size.height / 2
      - (bounds.y + bounds.height / 2) * zoom,
  };
}
```

效果：

```text
内容再偏、节点再远
     ↓
点击适应内容
     ↓
所有节点完整出现在当前画布中央
```

---

# 10. 实现“定位”

“定位”和“适应内容”不能重复。

我建议定义成：

## 有选中节点

定位选中的节点。

```text
选中节点保持当前 zoom
节点中心 -> 画布中心
```

## 没有选中节点

定位整个内容中心，但**保持当前 zoom**。

区别：

```text
定位
= 不改用户当前缩放，只改 x/y

适应内容
= x/y/zoom 都计算
```

这就是两个按钮最自然的行为。

---

## 10.1 `centerBoundsInViewport`

```ts
export function centerBoundsInViewport(
  bounds: FlowRect,
  size: Readonly<{
    width: number;
    height: number;
  }>,
  zoom: number,
): ViewportTransform {
  return {
    zoom,
    x:
      size.width / 2
      - (bounds.x + bounds.width / 2) * zoom,
    y:
      size.height / 2
      - (bounds.y + bounds.height / 2) * zoom,
  };
}
```

---

# 11. `FlowCanvasTools` 应该接收真实 canvasSize

现在 `FlowCanvas.tsx` 已经有：

```ts
const canvasSize = useCanvasSize(containerRef);
```

所以把：

```tsx
<FlowCanvasTools
  mode={toolMode}
  onModeChange={setToolMode}
/>
```

改成：

```tsx
<FlowCanvasTools
  canvasSize={canvasSize}
  mode={toolMode}
  onModeChange={setToolMode}
/>
```

然后 `FlowCanvasTools.tsx`：

```ts
const nodes =
  useFlowStore((state) => state.nodes);

const selectedNodeIds =
  useFlowStore((state) => state.selectedNodeIds);

const viewport =
  useFlowStore((state) => state.viewport);

const setViewport =
  useFlowStore((state) => state.setViewport);
```

### 定位

```ts
const locate = () => {
  const selected = nodes.filter((node) =>
    selectedNodeIds.has(node.id),
  );

  const targetNodes =
    selected.length > 0
      ? selected
      : nodes;

  const bounds =
    getNodesBounds(targetNodes);

  if (!bounds) return;

  setViewport(
    centerBoundsInViewport(
      bounds,
      canvasSize,
      viewport.zoom,
    ),
  );
};
```

### 适应内容

```ts
const fitContent = () => {
  const bounds =
    getNodesBounds(nodes);

  if (!bounds) return;

  setViewport(
    fitBoundsToViewport(
      bounds,
      canvasSize,
      {
        padding: 72,
        maxZoom: MAX_CANVAS_ZOOM,
      },
    ),
  );
};
```

---

# 12. 视口变化建议增加 180~220ms 过渡

如果点击“定位”以后 viewport 瞬移：

```text
x/y 直接跳
```

用户容易丢失方位感。

可以在 canvas transform 层增加：

```text
programmatic viewport transition
```

但是不要让用户拖动画布时也有 transition。

推荐：

```ts
type ViewportMotion =
  | 'direct'
  | 'animated';
```

或者第一版更简单：

```text
先不做动画
```

把数学行为修正确更重要。

如果做：

```text
180ms cubic-bezier(.2,.8,.2,1)
```

就够了。

---

# 13. 日志：协议保持英文枚举，产品展示全部中文

这块不要把：

```text
ExecutionEventKind
```

改成中文 enum。

协议应该继续稳定：

```text
workflow_started
node_started
backend_selected
...
```

真正应该变的是：

```text
ExecutionEvent
      ↓
ExecutionLogViewModel
      ↓
中文 UI
```

---

# 14. 新增日志展示适配层

建议新增：

```text
src/components/workflow/executionLogPresentation.ts
```

或者更干净一点：

```text
src/features/workflow/executionEventPresentation.ts
```

推荐：

```ts
export type ExecutionLogEntry = {
  sequence: number;
  nodeId: string | null;
  nodeLabel: string | null;
  eventLabel: string;
  detail: string;
  severity:
    | 'normal'
    | 'success'
    | 'warning'
    | 'error';
};
```

---

# 15. `ExecutionEventKind` 中文映射

推荐：

```ts
const EVENT_LABELS: Record<
  ExecutionEventKind,
  string
> = {
  workflow_started: '流程开始',
  node_started: '节点开始',
  log: '日志',
  node_output_produced: '节点输出',
  resource_acquired: '资源就绪',
  backend_selected: '执行引擎',
  command_exited: '命令结束',
  diagnostic_evidence_captured: '失败现场',
  node_succeeded: '节点完成',
  edge_traversed: '流程流转',
  node_failed: '节点失败',
  workflow_completed: '流程完成',
  workflow_failed: '流程失败',
};
```

UI 不再出现：

```text
node_succeeded
```

而出现：

```text
节点完成
```

---

# 16. 日志不要主要展示 node ID，要展示节点名称

当前：

```text
[read_search_value_1]
```

用户不应该每次都从 ID 反推：

```text
这是哪个节点？
```

建议：

```text
[读取搜索内容]
```

开发者信息可以弱化：

```text
读取搜索内容
read_search_value_1
```

或者 hover tooltip：

```text
节点 ID：read_search_value_1
```

---

# 17. 同一节点日志使用稳定颜色

你提到：

> 同一节点的输出搞个颜色区分，读起来很费劲

这个方向是对的。

但不要做：

```text
node_started = 灰
backend_selected = 青
node_succeeded = 绿
```

导致同一个节点的三条日志视觉上分散。

建议做“双颜色体系”：

```text
节点颜色
= 同一节点统一

事件状态颜色
= icon / 文案语义
```

例如：

```text
┌ 蓝色竖条 ──────────────────────────────────
│ 读取搜索内容   节点开始     正在执行
│ 读取搜索内容   执行引擎     已选择 Windows UIA
│ 读取搜索内容   节点完成     执行成功
└───────────────────────────────────────────
```

下一节点：

```text
┌ 紫色竖条 ──────────────────────────────────
│ 调试输出       节点开始
│ 调试输出       日志         UIA
│ 调试输出       节点完成
└───────────────────────────────────────────
```

这样看运行链就非常清楚。

---

# 18. 节点颜色如何稳定生成

不要真的随机。

方案一：

```text
按 Node kind 复用节点卡片颜色
```

这是我更推荐的。

例如：

```text
UI 节点      cyan
Condition    violet
Application  indigo
Command      slate
Debug        fuchsia
```

优点：

```text
画布和日志颜色一致
```

用户看到青色日志，回画布很容易找到对应 UI 节点。

如果同类节点很多，再加一个很弱的 hash 变体即可。

---

# 19. 日志样式建议

当前是：

```text
01 node_started  [read...] UI Click
02 backend_selected ...
03 node_succeeded ...
```

建议：

```text
01  流程开始
    用 UIA 驱动 Notepad++ 查找

02  ● 读取搜索内容
    节点开始
    正在执行

03  ● 读取搜索内容
    执行引擎
    Windows UI Automation

04  ✓ 读取搜索内容
    节点完成

05  ● 调试输出
    节点开始

06  ● 调试输出
    日志
    UIA

07  ✓ 调试输出
    节点完成

08  流程完成
    工作流执行完成
```

不一定要真的两行。

高密度版可以：

```text
02  读取搜索内容   节点开始     正在执行
03  读取搜索内容   执行引擎     Windows UI Automation
04  读取搜索内容   节点完成
```

我建议桌面 IDE 用高密度一行模式。

---

# 20. Backend 名称也应该产品化

当前 payload：

```ts
backend:
  | 'windows_uia'
  | 'browser_cdp'
  | 'visual_cache'
  | 'ocr_tiny'
  | 'ocr_medium'
  | 'gui_grounding'
  | 'send_input';
```

产品显示：

```ts
const BACKEND_LABELS: Record<
  BackendKind,
  string
> = {
  windows_uia:
    'Windows UI Automation',
  browser_cdp:
    '浏览器 CDP',
  visual_cache:
    '视觉缓存',
  ocr_tiny:
    'OCR 快速识别',
  ocr_medium:
    'OCR 精确识别',
  gui_grounding:
    '视觉定位',
  send_input:
    '模拟输入',
};
```

而不是把：

```text
windows_uia
```

直接显示给用户。

---

# 21. 用户自己写的 Log / Debug 内容不要翻译

这个要区分。

系统生成：

```text
node_started
node_succeeded
backend_selected
```

要中文化。

用户 Log 节点：

```text
订单号 = 123456
```

必须原样展示。

Debug 节点：

```text
UIA
```

也必须原样展示。

所以：

```text
事件标题
= UI 本地化

event.message
= 根据事件类型决定是否使用 / 原样展示
```

不要把所有 message 做字符串替换。

---

# 22. 后端 `node_label()` 怎么处理

现在 Rust：

```rust
Start
Log
Debug Output
UI Click
UI SetValue
...
```

有两个选择。

## 方案 A：前端不再依赖它做主标题

推荐。

`node_started.message` 可以继续留着作为技术 detail。

用户主标题来自当前 Workflow 节点：

```text
node_id
   ↓
Flow store
   ↓
node.data.label
```

优点：

```text
用户给节点改名字以后，日志自动跟着新名字
```

这比 Runtime 自己猜“UI Click”更合理。

---

## 方案 B：直接把 Rust `node_label()` 改中文

可以做，但只能解决当前中文 UI。

未来如果有英文界面：

```text
Runtime 又要反过来改
```

所以我建议：

> Rust 事件继续是结构化事实；展示语言交给 UI。

---

# 23. 复制日志也应该复制“人类可读版”

当前：

```ts
const completeLog =
  events.map(formatEvent).join('\n');
```

`formatEvent()` 又输出 raw kind。

建议复制：

```text
01 流程开始 用 UIA 驱动 Notepad++ 查找
02 [读取搜索内容] 节点开始
03 [读取搜索内容] 执行引擎 Windows UI Automation
04 [读取搜索内容] 节点完成
...
```

同时可以在“日志设置 / 开发者模式”里以后提供：

```text
复制原始事件 JSON
```

但默认一定是中文可读日志。

---

# 24. 顶部工具栏：把第二行真正删掉

这次不要：

```text
看起来像放到顶部
但 EditorCommandBar 组件还占一行透明高度
```

而要真正改 `App.tsx` grid。

当前：

```ts
appView === 'home'
  ? 'grid-rows-[40px_minmax(0,1fr)_40px]'
  : 'grid-rows-[40px_40px_minmax(0,1fr)_40px]'
```

建议：

```ts
'grid-rows-[40px_minmax(0,1fr)_40px]'
```

Home 和 Editor 都三行。

---

# 25. `EditorCommandBar` 不应该继续是一个完整 `<nav>`

建议拆成：

```text
EditorCommandBar.tsx
    ↓

EditorToolbarControls.tsx
EditorRunControls.tsx
```

或者：

```tsx
<EditorCommands />
<EditorPrimaryActions />
```

这样 `WindowTitleBar` 可以自由组合。

推荐结构：

```text
src/components/workflow/
  EditorToolbarControls.tsx
  EditorPrimaryActions.tsx

src/components/shell/
  WindowTitleBar.tsx
```

---

# 26. 顶部布局建议

40px 内：

```text
┌───────────────────────────────────────────────────────────────────────┐
│ Logo  Workspace  Workflow  Saved │ Undo Redo Copy ... │ 校验 运行 发布 │ Search │ Service │ Win │
└───────────────────────────────────────────────────────────────────────┘
```

具体可以：

```text
Left
├── Logo
├── 默认工作区
├── 当前 Workflow
└── 保存/运行状态

Center
├── Undo
├── Redo
├── Copy
├── Paste
├── Duplicate
├── Delete
├── Library
├── Console
└── Inspector

Right
├── Validate
├── Run
├── Publish
├── Search
├── Service
├── Bell
├── Help
└── Window controls
```

---

# 27. 窗口拖拽区域要注意

当前 `WindowTitleBar.tsx`：

```ts
if (
  target instanceof HTMLElement
  && target.closest(
    'button, input, select'
  )
) return;
```

所以按钮并入 TitleBar 后不会导致误拖窗口。

这套机制可以继续使用。

但建议把：

```text
所有真实按钮
```

都保持为 `<button>`，不要用 div + onClick。

---

# 28. 小窗口宽度下要有降级策略

顶部塞进更多命令后需要考虑宽度。

推荐优先级：

```text
必须保留：
Run
Window controls

高优先：
Validate
Workflow name
Undo / Redo

中优先：
Copy / Paste / Delete
Panel toggles

低优先：
Search
Publish
Service text
```

窄屏时：

```text
Search
```

可以只保留图标。

或者：

```text
< 1400px
隐藏搜索输入框
显示 Search icon
```

不要让窗口控制按钮被挤掉。

---

# 29. 推荐的组件接口

我建议 `WindowTitleBar` 不要收到 20 个 command callback。

可以给它一个 React slot：

```ts
type WindowTitleBarProps = {
  workflowName: string;
  running: boolean;
  report: ValidationReport | null;
  errorMessage: string | null;

  editorCommands?: ReactNode;
  editorActions?: ReactNode;

  ...
};
```

App：

```tsx
<WindowTitleBar
  workflowName={studio.workflowName}
  running={studio.running}
  report={studio.report}
  errorMessage={studio.errorMessage}
  editorCommands={
    appView === 'editor' ? (
      <EditorToolbarControls
        store={studio.flowStore}
        libraryOpen={libraryOpen}
        inspectorOpen={inspectorOpen}
        consoleOpen={consoleOpen}
        onToggleLibrary={toggleLibrary}
        onToggleInspector={toggleInspector}
        onToggleConsole={toggleConsole}
      />
    ) : null
  }
  editorActions={
    appView === 'editor' ? (
      <EditorPrimaryActions
        running={studio.running}
        onValidate={() =>
          void studio.validate()
        }
        onRun={() =>
          void studio.run()
        }
      />
    ) : null
  }
  ...
/>
```

这样：

```text
Shell
```

负责布局，

```text
Workflow
```

仍然负责业务命令。

不会把 Flow store 直接耦合进 Shell。

---

# 30. 文件级改动清单

## 必改

```text
src/App.tsx
```

改：

- 移除独立 `EditorCommandBar` 行
- editor grid 从 4 行变 3 行
- 把 command/action controls 传入 `WindowTitleBar`
- `WorkflowWorkspace` 额外拿到 node metadata 或 store，供日志展示节点名称

---

```text
src/components/shell/WindowTitleBar.tsx
```

改：

- 增加 editor command slot
- 增加 validate/run/publish slot
- 调整 left / center / right 布局
- 响应窄窗口
- 保留 Windows 拖动 / 最小化 / 最大化 / 关闭

---

```text
src/components/workflow/EditorCommandBar.tsx
```

建议：

- 拆掉外层 40px `<nav>`
- 拆分为：
  - `EditorToolbarControls`
  - `EditorPrimaryActions`

如果不想改文件名，也可以让：

```text
EditorCommandBar
```

变成只返回 controls，不再负责整行布局。

---

```text
src/features/workflow/workflowModel.ts
```

改：

```ts
NodeRunState
```

新增：

```text
pending
skipped
```

并补完整 event state transition。

---

```text
src/features/workflow/useWorkflowStudio.ts
```

改：

- validate 成功后，把全部节点标记 pending
- `workflow_completed / workflow_failed` 时让 pending -> skipped
- 保留 `edge_traversed -> activateEdge`
- 如果做 traversedEdgeIds，在这里一并更新

---

```text
src/components/workflow/WorkflowNodeCard.tsx
```

改：

- 运行态从小圆点升级为卡片级视觉反馈
- running 呼吸边框
- success check
- pending / skipped 状态
- 保留原节点 type accent

---

```text
src/flow/FlowEdges.tsx
```

改：

- 去掉 `ActiveEdgeParticles`
- 新增 `ActiveEdgePulse`
- 用 `strokeDashoffset` 实现电流脉冲
- 保留 activeEdgeIds 多边并发短暂显示

---

```text
src/flow/FlowCanvas.tsx
```

改：

```tsx
<FlowCanvasTools
  canvasSize={canvasSize}
  ...
/>
```

---

```text
src/flow/FlowCanvasTools.tsx
```

改：

- 删除 `resetViewport`
- 实现 `locate`
- 实现 `fitContent`

---

```text
src/components/workflow/ExecutionLog.tsx
```

改：

- 不再直接显示 `event.kind`
- 不再以 node_id 作为主要识别信息
- 同一节点稳定颜色
- 中文事件名称
- 中文 backend 名称
- 复制中文人类可读日志

---

## 建议新增

```text
src/flow/viewport.ts
```

包含：

```text
getNodesBounds
centerBoundsInViewport
fitBoundsToViewport
```

---

```text
src/flow/viewport.test.ts
```

测试纯数学。

---

```text
src/features/workflow/executionEventPresentation.ts
```

包含：

```text
EVENT_LABELS
BACKEND_LABELS
resolveExecutionLogEntry
resolveNodeLogTone
formatExecutionEventForClipboard
```

---

# 31. 运行时展示最好逐步从 Document State 中独立出去

当前：

```text
runState
invalid
```

放在：

```ts
WorkflowNodeData
```

里。

虽然你使用：

```ts
setNodes(..., false)
```

不会写历史，

而且：

```ts
toWorkflowDefinition()
```

也不会序列化 runState，

所以短期没有大问题。

但长期更干净的模型应该是：

```text
Flow document
  ├── nodes
  ├── edges
  └── metadata

Flow runtime presentation
  ├── nodeStates
  ├── activeEdgeIds
  ├── traversedEdgeIds
  └── runId
```

也就是：

```ts
runtime: {
  nodeStates:
    Record<string, NodeRunState>;

  activeEdgeIds:
    Record<string, number>;

  traversedEdgeIds:
    Set<string>;
}
```

这样：

```text
编辑数据
```

和：

```text
一次运行的瞬时状态
```

完全分离。

不过这次如果想控制改动量，我不建议强制一起重构。

第一版继续用现有 `runState` 即可。

---

# 32. CSS 动画建议集中管理

不要在组件里到处写不同的 arbitrary animation。

在：

```text
src/styles.css
```

或：

```text
src/theme.css
```

集中放：

```css
@keyframes argus-node-running {
  0%,
  100% {
    opacity: 0.35;
    transform: scale(1);
  }

  50% {
    opacity: 0.9;
    transform: scale(1.015);
  }
}

@keyframes argus-edge-current {
  to {
    stroke-dashoffset: -26;
  }
}
```

然后：

```text
node running ring
edge current
```

统一命名。

---

# 33. 必须支持 reduced motion

这属于桌面工具该有的基本行为。

Edge：

```tsx
className="motion-reduce:hidden"
```

Node：

```text
motion-reduce:animate-none
```

关闭动画时仍必须有：

```text
蓝色 running border
绿色 success icon
红色 error border
```

状态不能依赖动画才能看出来。

---

# 34. 日志建议自动滚动，但只在用户停留底部时滚

当前运行日志实时追加。

建议：

```text
用户正在看最底部
    ↓
新日志自动 scrollIntoView

用户已经手动向上翻
    ↓
不要强行把他拉回底部
```

可以：

```text
distanceFromBottom < 40px
```

时才 auto-scroll。

同时右下角显示：

```text
↓ 3 条新日志
```

这个可以放后续阶段。

---

# 35. 日志分组可以做，但第一版不要折叠

你说：

> 同一节点输出读起来很费劲

第一版：

```text
同节点颜色 + 节点名称
```

已经能明显改善。

暂时不要一上来做：

```text
节点可折叠 group
```

因为运行时用户需要看到真实时间顺序：

```text
node
edge
node
edge
```

如果折叠，会打乱“发生了什么”的直觉。

---

# 36. 测试计划

这次一定要补测试，否则 viewport 和事件状态很容易回归。

---

## 36.1 `viewport.test.ts`

### 空节点

```text
getNodesBounds([])
-> null
```

### 单节点

```text
node:
x=100
y=200
w=120
h=50
```

应：

```text
bounds:
100, 200, 120, 50
```

### 多节点负坐标

```text
A:
-300,-100,100,50

B:
500,400,200,100
```

应正确得到全局包围盒。

### fit 不超过最大缩放

```text
极小内容
```

zoom 不得 > `MAX_CANVAS_ZOOM`。

### fit 超大内容

```text
所有节点必须进入 viewport padding 内
```

---

## 36.2 `FlowCanvasTools.test.tsx`

测试：

```text
点击定位
```

有选中：

```text
viewport 中心 -> 选中节点中心
zoom 不变
```

没有选中：

```text
viewport 中心 -> 全部节点 bounds 中心
zoom 不变
```

点击适应内容：

```text
x / y / zoom 都变化
```

必须明确防止回归成：

```ts
{x:0,y:42,zoom:1}
```

---

## 36.3 `workflowModel.test.ts`

事件转换：

```text
workflow_started
idle -> pending

node_started
pending -> running

node_succeeded
running -> success

node_failed
running -> error

workflow_completed
pending -> skipped
success -> success
error -> error
```

---

## 36.4 `WorkflowNodeCard.test.tsx`

断言：

```text
pending:
显示“待运行”

running:
显示“正在运行”
有 running data-state

success:
显示“已完成”

error:
显示“失败”

skipped:
显示“未执行”
```

建议加：

```tsx
data-run-state={status}
```

测试会更稳，不要只断言 CSS class。

---

## 36.5 `FlowEdges.test.tsx`

收到 active edge：

```text
存在 pulse path
```

普通 edge：

```text
不存在 runtime pulse path
```

可以给：

```tsx
data-flow-edge-runtime="active"
```

---

## 36.6 `ExecutionLog.test.tsx`

现在已有测试，建议扩充：

输入：

```ts
node_started
backend_selected
node_succeeded
```

应显示：

```text
节点开始
执行引擎
节点完成
```

不再显示：

```text
node_started
backend_selected
node_succeeded
```

同一 node_id：

```text
相同 data-node-tone
```

不同 node：

```text
不同 node identity
```

---

## 36.7 `WindowTitleBar.test.tsx`

验证：

```text
编辑器模式：
能看到 undo / run

Home：
不显示编辑命令

运行中：
Run disabled
显示运行中

窗口拖拽：
点击 button 不触发 startDragging
```

---

# 37. 推荐实施顺序

我建议分四个 commit。

---

## Commit 1：修正常规画布操作

```text
feat(flow): implement locate and fit-content viewport tools
```

改：

```text
src/flow/viewport.ts
src/flow/viewport.test.ts
src/flow/FlowCanvas.tsx
src/flow/FlowCanvasTools.tsx
src/flow/FlowCanvasTools.test.tsx
```

这是纯 Flow 改动，最独立。

---

## Commit 2：运行节点和连线动效

```text
feat(workflow): improve runtime node and edge visual states
```

改：

```text
workflowModel.ts
useWorkflowStudio.ts
WorkflowNodeCard.tsx
FlowEdges.tsx
styles.css / theme.css
tests
```

先把：

```text
pending
running
success
error
skipped
```

跑完整。

---

## Commit 3：日志中文化与同节点视觉分组

```text
feat(workflow): localize and group execution logs
```

改：

```text
executionEventPresentation.ts
ExecutionLog.tsx
WorkflowWorkspace.tsx
WorkflowConsolePanel.tsx
tests
```

核心：

```text
协议不改
展示中文
节点名优先
同节点颜色一致
```

---

## Commit 4：合并顶部命令栏

```text
refactor(shell): merge editor commands into title bar
```

改：

```text
App.tsx
WindowTitleBar.tsx
EditorCommandBar.tsx
tests
```

最后做这个，避免布局重构和运行态功能同时搅在一起。

---

# 38. 验收标准

完成以后，我认为下面全部通过才算这次方案完成。

## 节点

- [ ] 点击运行后，所有可执行节点立即进入“待运行”
- [ ] 当前节点明显显示“正在运行”
- [ ] 正在运行节点有蓝色呼吸边框，但不移动节点位置
- [ ] 完成节点显示绿色完成状态
- [ ] 失败节点显示红色失败状态
- [ ] 条件分支未执行节点在流程结束后变为“未执行”
- [ ] 上一次运行状态在下一次运行前被正确重置

## 连线

- [ ] `edge_traversed` 会触发对应 edge 的电流脉冲
- [ ] 脉冲方向从 source -> target
- [ ] 快速连续 edge 不会因为单 active id 互相覆盖
- [ ] 动画结束后 edge 恢复正常样式
- [ ] reduced-motion 下不播放动画但状态仍可辨识

## 画布

- [ ] “定位”不再 reset 到 `{0,42,1}`
- [ ] 有选中节点时，“定位”把选中节点居中
- [ ] 无选中节点时，“定位”把全部内容中心居中
- [ ] “定位”保持当前 zoom
- [ ] “适应内容”把全部节点完整放进当前画布
- [ ] 适应内容自动计算 zoom
- [ ] 极端坐标、负坐标、超大图仍工作
- [ ] 空画布点击按钮不报错

## 日志

- [ ] UI 不显示 `node_started / node_succeeded` 等 raw enum
- [ ] 显示“流程开始 / 节点开始 / 节点完成 / 流程完成”
- [ ] node_id 不再作为主要可读名称
- [ ] 同一个节点的日志有一致颜色
- [ ] 不同节点在连续日志中明显可区分
- [ ] Backend 名称产品化
- [ ] 用户 Log / Debug 文本原样保留
- [ ] “复制日志”复制中文可读版
- [ ] 错误事件仍使用红色语义状态，不被 node color 淹没

## 顶部布局

- [ ] 编辑器不再有独立第二行 CommandBar
- [ ] Undo / Redo / Copy / Paste / Delete 可以在 TitleBar 使用
- [ ] 校验 / 运行 / 发布位于 TitleBar
- [ ] Run 运行中正确 disabled
- [ ] Search / 服务状态 / 窗口控制仍可用
- [ ] 标题栏空白区域仍可拖动窗口
- [ ] 按钮区域不会触发窗口拖动
- [ ] 小窗口不会把关闭按钮挤出屏幕

---

# 39. 最终效果建议

运行前：

```text
○ Start ───── ○ 打开应用 ───── ○ 读取文本 ───── ○ Debug ───── ○ End
```

点击运行：

```text
○ Start ───── ○ 打开应用 ───── ○ 读取文本 ───── ○ Debug ───── ○ End
  待运行         待运行             待运行          待运行        待运行
```

Start：

```text
◉ Start ▰▰▰▰▰ ○ 打开应用 ───── ○ 读取文本 ───── ○ Debug ───── ○ End
正在运行
```

打开应用：

```text
✓ Start ═════ ◉ 打开应用 ▰▰▰▰ ○ 读取文本 ───── ○ Debug ───── ○ End
已完成          正在运行
```

读取：

```text
✓ Start ═════ ✓ 打开应用 ═════ ◉ 读取文本 ▰▰▰▰ ○ Debug ───── ○ End
```

完成：

```text
✓ Start ═════ ✓ 打开应用 ═════ ✓ 读取文本 ═════ ✓ Debug ═════ ✓ End
```

Condition 未命中分支：

```text
✓ Condition
     │
     ├════ ✓ A
     │
     └──── – B
            未执行
```

底部日志：

```text
01  流程开始          用 UIA 驱动 Notepad++ 查找

02  开始              节点开始
03  开始              节点完成

04  读取搜索内容       节点开始
05  读取搜索内容       执行引擎      Windows UI Automation
06  读取搜索内容       节点输出      已产生 1 个值输出
07  读取搜索内容       节点完成

08  调试输出           节点开始
09  调试输出           日志          UIA
10  调试输出           节点完成

11  流程完成          工作流执行完成
```

这套信息密度仍然是 IDE 风格，但读起来已经不再像后端 trace。

---

# 40. 这次不建议做的东西

为了控制范围，这次不要顺手做：

```text
完整运行历史数据库
可折叠日志树
断点调试
单步执行
暂停 / 恢复
运行时节点耗时图
Minimap
自动布局
日志全文搜索
运行回放
```

这些后面都值得做。

但是现在最应该先完成的是：

```text
运行中的流程
必须“看得懂它正在去哪”
```

也就是：

```text
节点状态
+
连线脉冲
+
可读日志
+
正确 viewport
```

先把这个闭环做到位。

---

# 41. 最推荐的最终边界

```text
┌─────────────────────────────────────────────┐
│                  Runtime                    │
│ ExecutionEvent / payload / stable protocol │
└─────────────────────┬───────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────┐
│         Workflow Runtime Presentation       │
│                                             │
│ node state                                  │
│ edge state                                  │
│ Chinese log presentation                    │
│ node identity / tone                        │
└─────────────────────┬───────────────────────┘
                      │
          ┌───────────┼────────────┐
          ▼           ▼            ▼
      NodeCard     FlowEdges    ExecutionLog
```

Flow 内核另一条边界：

```text
┌─────────────────────────────────────────────┐
│                Flow Engine                  │
│ nodes / edges / routing / viewport          │
└─────────────────────┬───────────────────────┘
                      │
                      ▼
              viewport helpers
          ┌───────────┴───────────┐
          ▼                       ▼
       locate                 fit content
```

Shell：

```text
WindowTitleBar
    ├── shell identity
    ├── document identity
    ├── editor commands slot
    ├── run actions slot
    └── window controls
```

这三个边界定下来以后，这次需求不会变成一堆“为了截图挪 UI 的补丁”，而是能继续支撑后续：

```text
单步执行
暂停
断点
运行回放
运行记录
节点耗时
失败定位
```

---

# 42. 一句话实施建议

我会直接按：

```text
先修 viewport
    ↓
补完整 NodeRunState
    ↓
把 edge 粒子升级成电流 pulse
    ↓
做 ExecutionEvent 中文 presentation layer
    ↓
最后把 EditorCommandBar 合进 WindowTitleBar
```

这个顺序落地。

**不需要重写 Runtime，也不需要改变现有 ExecutionEvent 协议；主要工作集中在前端 Flow / Workflow Presentation / Shell 三层。**
