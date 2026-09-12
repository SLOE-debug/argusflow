import type { FlowPoint } from "../types";
import type { RoutingObstacles } from "./obstacles";

/** 障碍边界与端点投影组成可见网格，按需扩展而非构造完整笛卡尔积。 */
interface SearchNode {
  readonly key: number;
  readonly x: number;
  readonly y: number;
  readonly axis: number;
  readonly cost: number;
  readonly score: number;
}
class Queue {
  private readonly entries: SearchNode[] = [];
  private earlier(a: SearchNode, b: SearchNode): boolean {
    return (
      a.score < b.score ||
      (a.score === b.score &&
        (a.cost < b.cost || (a.cost === b.cost && a.key < b.key)))
    );
  }
  push(item: SearchNode): void {
    let i = this.entries.length;
    this.entries.push(item);
    while (i > 0) {
      const parent = (i - 1) >>> 1;
      if (!this.earlier(item, this.entries[parent])) break;
      this.entries[i] = this.entries[parent];
      i = parent;
    }
    this.entries[i] = item;
  }
  pop(): SearchNode | undefined {
    const first = this.entries[0],
      last = this.entries.pop();
    if (!this.entries.length || !last) return first;
    let i = 0;
    while (i * 2 + 1 < this.entries.length) {
      let child = i * 2 + 1;
      if (
        child + 1 < this.entries.length &&
        this.earlier(this.entries[child + 1], this.entries[child])
      )
        child++;
      if (!this.earlier(this.entries[child], last)) break;
      this.entries[i] = this.entries[child];
      i = child;
    }
    this.entries[i] = last;
    return first;
  }
}
/** 有限预算保护主线程；失败必须显示受阻状态。 */
export function searchRoute(
  start: FlowPoint,
  end: FlowPoint,
  obstacles: RoutingObstacles,
  sourceNormal: FlowPoint,
  targetNormal: FlowPoint,
  cornerSpace = 10,
): readonly FlowPoint[] | null {
  const xs = new Set([
      start.x,
      end.x,
      start.x - 24,
      start.x + 24,
      end.x - 24,
      end.x + 24,
    ]),
    ys = new Set([
      start.y,
      end.y,
      start.y - 24,
      start.y + 24,
      end.y - 24,
      end.y + 24,
    ]);
  for (const rect of obstacles.rects) {
    xs.add(rect.x - cornerSpace);
    xs.add(rect.x + rect.width + cornerSpace);
    ys.add(rect.y - cornerSpace);
    ys.add(rect.y + rect.height + cornerSpace);
  }
  const xValues = [...xs].sort((a, b) => a - b),
    yValues = [...ys].sort((a, b) => a - b);
  const key = (x: number, y: number, axis: number) =>
    (x * yValues.length + y) * 3 + axis;
  const startX = xValues.indexOf(start.x),
    startY = yValues.indexOf(start.y),
    endX = xValues.indexOf(end.x),
    endY = yValues.indexOf(end.y);
  const costs = new Map<number, number>(),
    parents = new Map<number, number>();
  const queue = new Queue();
  const first = key(startX, startY, 2);
  costs.set(first, 0);
  queue.push({
    key: first,
    x: startX,
    y: startY,
    axis: 2,
    cost: 0,
    score: Math.abs(start.x - end.x) + Math.abs(start.y - end.y),
  });
  for (let expanded = 0; expanded < 50_000; expanded++) {
    const current = queue.pop();
    if (!current)
      return cornerSpace
        ? searchRoute(start, end, obstacles, sourceNormal, targetNormal, 0)
        : null;
    if (costs.get(current.key) !== current.cost) continue;
    if (current.x === endX && current.y === endY) {
      const path: FlowPoint[] = [];
      let cursor: number | undefined = current.key;
      while (cursor !== undefined) {
        const cell = Math.floor(cursor / 3);
        path.push({
          x: xValues[Math.floor(cell / yValues.length)],
          y: yValues[cell % yValues.length],
        });
        cursor = parents.get(cursor);
      }
      return path.reverse();
    }
    const from = { x: xValues[current.x], y: yValues[current.y] };
    for (const [dx, dy, axis] of [
      [1, 0, 0],
      [0, 1, 1],
      [-1, 0, 0],
      [0, -1, 1],
    ]) {
      const x = current.x + dx,
        y = current.y + dy;
      if (x < 0 || y < 0 || x >= xValues.length || y >= yValues.length)
        continue;
      const to = { x: xValues[x], y: yValues[y] };
      if (
        current.key === first &&
        dx * sourceNormal.x + dy * sourceNormal.y < 0
      )
        continue;
      if (
        x === endX &&
        y === endY &&
        dx * targetNormal.x + dy * targetNormal.y > 0
      )
        continue;
      if (!obstacles.clear(from, to)) continue;
      const cost =
        current.cost +
        Math.abs(from.x - to.x) +
        Math.abs(from.y - to.y) +
        (current.axis === 2 || current.axis === axis ? 0 : 24);
      const id = key(x, y, axis);
      if (cost >= (costs.get(id) ?? Infinity)) continue;
      costs.set(id, cost);
      parents.set(id, current.key);
      queue.push({
        key: id,
        x,
        y,
        axis,
        cost,
        score: cost + Math.abs(to.x - end.x) + Math.abs(to.y - end.y),
      });
    }
  }
  return null;
}
