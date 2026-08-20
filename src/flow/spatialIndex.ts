import type { FlowRect } from './types';

/** 固定单元格空间哈希，用于快速查询移动区域影响到的节点或路径。 */
export class SpatialHash<T> {
  private readonly cells = new Map<string, Set<T>>();
  private readonly bounds = new Map<T, FlowRect>();

  public constructor(private readonly cellSize = 160) {}

  public set(item: T, bounds: FlowRect): void {
    this.delete(item);
    this.bounds.set(item, bounds);
    for (const cell of this.cellsFor(bounds)) {
      const bucket = this.cells.get(cell) ?? new Set<T>();
      bucket.add(item);
      this.cells.set(cell, bucket);
    }
  }

  public delete(item: T): void {
    const bounds = this.bounds.get(item);
    if (!bounds) return;
    for (const cell of this.cellsFor(bounds)) this.cells.get(cell)?.delete(item);
    this.bounds.delete(item);
  }

  public query(bounds: FlowRect): Set<T> {
    const result = new Set<T>();
    for (const cell of this.cellsFor(bounds)) for (const item of this.cells.get(cell) ?? []) result.add(item);
    return result;
  }

  private cellsFor(bounds: FlowRect): string[] {
    const result: string[] = [];
    const left = Math.floor(bounds.x / this.cellSize);
    const right = Math.floor((bounds.x + bounds.width) / this.cellSize);
    const top = Math.floor(bounds.y / this.cellSize);
    const bottom = Math.floor((bounds.y + bounds.height) / this.cellSize);
    for (let x = left; x <= right; x += 1) for (let y = top; y <= bottom; y += 1) result.push(`${x}:${y}`);
    return result;
  }
}
