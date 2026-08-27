import { SpatialHash } from './spatialIndex';
import type { FlowRect } from '../types';

/**
 * 使用稳定 ID 管理 SpatialHash 中的对象身份。
 *
 * 调用方可以在更新时创建新值，包装层会先删除旧对象，避免临时对象 identity
 * 让动态障碍物或路径线段残留在旧桶中。
 */
export class SpatialIndexById<TId, TValue> {
  /** 稳定 ID 对应的当前条目。 */
  private readonly entries = new Map<TId, TValue>();
  /** 真正执行局部矩形查询的空间哈希。 */
  private readonly spatialHash: SpatialHash<TValue>;

  /** 使用指定世界像素单元格尺寸创建稳定 ID 空间索引。 */
  public constructor(cellSize = 160) {
    this.spatialHash = new SpatialHash<TValue>(cellSize);
  }

  /** 插入或替换一个稳定 ID 条目。 */
  public set(id: TId, value: TValue, bounds: FlowRect): void {
    const previous = this.entries.get(id);
    if (previous !== undefined) this.spatialHash.delete(previous);
    this.entries.set(id, value);
    this.spatialHash.set(value, bounds);
  }

  /** 删除稳定 ID 条目并同步清理空间桶。 */
  public delete(id: TId): boolean {
    const previous = this.entries.get(id);
    if (previous === undefined) return false;
    this.spatialHash.delete(previous);
    return this.entries.delete(id);
  }

  /** 读取一个稳定 ID 的当前值。 */
  public get(id: TId): TValue | undefined {
    return this.entries.get(id);
  }

  /** 查询与矩形所在哈希桶重叠的候选值。 */
  public query(bounds: FlowRect): ReadonlySet<TValue> {
    return this.spatialHash.query(bounds);
  }

  /** 返回当前全部稳定 ID。 */
  public ids(): IterableIterator<TId> {
    return this.entries.keys();
  }

  /** 返回当前全部值；只允许遍历，不暴露内部 Map。 */
  public values(): IterableIterator<TValue> {
    return this.entries.values();
  }
}
