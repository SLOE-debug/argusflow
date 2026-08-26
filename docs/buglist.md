我看了当前 `main` 的最新提交，还是 **`fe301cb` — `fix: 修正画布智能避障与编辑器布局`**。这次改动确实加强了端点碰撞和网格寻路，但从你这两张图看，**这两个视觉结果都不应该作为最终预期行为接受**。

第一张这种拖动时的**阶梯/锯齿线，不是预期**，这是当前 preview fallback 的算法退化。现在拖动端点后，`previewEdgeRoute()` 会先尝试少量 L/Z/外围正交候选；一旦这些候选被挡住，就退到 `findGridPathAgainstRects()`，最终跑固定 **20px 网格 A***。所以你看到的实际上是一个类似：

```text
────┐
    └─┐
      └─┐
        └─┐
          └────
```

的网格路径，再被 `roundedPath()` 给每个小折点加圆角，于是就形成截图里的“圆润锯齿”。当前寻路器确实是 `GRID_SIZE = 20`、四方向扩展，并额外计算 `TURN_COST`。

这里甚至还有一个更具体的算法问题：**A* 的 cost 与进入方向有关，但 closed/bestCosts 却只按 `(x,y)` 存。** `GridNode` 明明带有 `direction`，转弯又有额外成本，但 `bestCosts` 和 `closed` 使用的只是 `pointKey(point)`。 这在图论上意味着状态压错了：

```text
(x, y, 从左边进入)
```

和

```text
(x, y, 从上边进入)
```

后续成本并不相同，却被当成同一个状态。这会让路径质量变差，也可能错误剪枝。正确的话至少应该是：

```ts
key = `${x},${y},${direction}`
```

不过即使把这个 bug 修了，**我也不建议把固定网格 A* 继续作为拖动实时算法**。正常编辑器里拖节点时，线应该最多出现几个稳定的 90° 折点，而不是暴露网格搜索痕迹。

第二张这种**线路贴着 node 绕一圈，也不是我认为合理的产品预期**；但它倒是符合当前代码的实现逻辑，所以这不是渲染偶发现象。你现在故意把普通节点和端点节点区别处理：

```text
普通中间节点
→ nodeObstacle()
→ 外扩 OBSTACLE_GAP = 16

source / target 节点
→ 只拿真实 node rect
→ 不使用 16px 外扩
```

同时端点只强制先走 `ENDPOINT_CLEARANCE = 14` 再允许转弯。也就是说现在的策略实际上是：

```text
普通障碍节点：至少留 16px
当前连线自己的 source/target：只保证离开端口 14px
```

所以路由主体**天然有机会非常靠近自己的 source/target 节点**。当前 `routing.ts` 的注释甚至明确写的是“起止节点使用真实边界，其余节点保留膨胀后的避障安全区”。

而且你的矩形碰撞是包含边界接触的：

```ts
a.x <= b.x + b.width
&& a.x + a.width >= b.x
...
```

因此它虽然一般不会真正穿进节点，但算法会把“刚好沿安全边缘走”当成非常便宜的合法路线。

我会把这两张图直接定义成两个需要修的路由质量 bug：

1. **P0 — `grid staircase leakage`**：拖动预览不得把 20px 网格搜索过程直接表现为连续小折点。先把 A* 状态改为 `(point, direction)`；更重要的是 move 热路径换成 previous-route repair / sparse OVG。
2. **P0 — `endpoint hugging`**：source/target 也必须有明确的 **port tunnel + body keep-out zone**。只允许 `anchor → escape` 这小段位于节点安全区里；一旦到 escape，主体 route 就必须和节点保持统一的视觉 clearance，例如 **16~20px**。

理想效果应该类似：

```text
           ┌──────── node ────────┐
           │                      │
───────────●                      │
 port      │                      │
           └──────────────────────┘
           ◄── 16~20px keep-out ──►
```

而不是：

```text
┌──────── node ────────┐
│                      │
└──────────────────────┘────┐
╰───────────────────────────╯
```

所以结论很明确：**第一张是算法退化，不应出现；第二张是当前策略会产生的结果，但设计上也不应该接受。** 你上一版 V2 方案里我会把这两个直接提升为 P0 验收项：**拖动不得出现 stair-step；除合法端口 tunnel 外，任何 route segment 都不得进入 endpoint 的 inflated keep-out rect。**
