import type { InspectionRect } from './inspectionContracts';

/** 独立屏幕帧；时间为相对录制起点的微秒，各来源分别排序。 */
export type ScreenFrame = Readonly<{
  id: number; source: number; generation: number; revision: number;
  presented_us: number; frozen_us: number; bounds: InspectionRect;
  previous: number | null; changes: readonly InspectionRect[];
  patches: readonly Readonly<{ index: number; bounds: InspectionRect }>[];
}>;
/** 只发布已成功落盘的有效前缀。 */
export type ScreenTimeline = Readonly<{
  /** 录制实际时长，去重后保留静止尾段。 */
  duration_us: number;
  /** 候选像素保留与离线 GPU 精确化状态。 */
  refinement: Readonly<{ state: 'pending' | 'complete' }> | Readonly<{ state: 'failed'; message: string }>;
  diagnostics: Readonly<{ queue_peak_items: number; queue_peak_bytes: number; readback_bytes: number; diff_total_us: number }>;
  frames: readonly ScreenFrame[];
  completeness: Readonly<{ state: 'complete' | 'disabled' }> | Readonly<{
    state: 'incomplete'; reason: 'history_gap' | 'unavailable' | 'closed' | 'capacity' | 'accumulated_frames' | 'storage';
  }>;
}>;
/** 时间关联不代表输入与变化的因果关系。 */
export type EventScreenEvidence = Readonly<{ before: readonly number[]; after: readonly number[] }>;
