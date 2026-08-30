import type {
  StartupComponentLifecycle,
  StartupPhase,
  StartupSnapshot,
} from './model';

/** 启动阶段对应的简短标题与补充说明。 */
export const STARTUP_PHASE_COPY: Readonly<Record<StartupPhase, Readonly<{
  title: string;
  detail: string;
}>>> = {
  starting_runtime: {
    title: '正在准备本地运行环境',
    detail: '屏幕捕获和两套 OCR 模型正在同时启动。',
  },
  initializing_capture: {
    title: '正在启动屏幕捕获',
    detail: '正在连接 Windows 图形设备；OCR 模型也在并行准备。',
  },
  selecting_ocr_device: {
    title: '正在检测 OCR 加速设备',
    detail: '检测到可用 GPU 时会优先启用，否则自动使用 CPU。',
  },
  loading_small_model: {
    title: '正在加载快速 OCR',
    detail: '快速与精确模型正在并行加载，完成后将自动进入工作台。',
  },
  warming_small_model: {
    title: '正在优化首次识别',
    detail: '正在完成首次识别优化；精确模型也在同步预热。',
  },
  loading_medium_model: {
    title: '正在加载精确 OCR',
    detail: '正在准备复杂页面识别模型，其他能力无需等待它启动。',
  },
  warming_medium_model: {
    title: '正在预热精确 OCR',
    detail: '最后一项能力就绪后将自动进入工作台。',
  },
  ready: {
    title: '准备完成',
    detail: '全部桌面能力已经可以使用。',
  },
  failed: {
    title: '部分能力未能启动',
    detail: '可以重试，或进入工作台继续编辑和检查流程。',
  },
};

/** 单项生命周期对应的用户可读状态。 */
export const COMPONENT_STATUS_LABELS: Readonly<Record<StartupComponentLifecycle, string>> = {
  pending: '等待中',
  initializing: '初始化中',
  warming: '预热中',
  ready: '已就绪',
  failed: '未启动',
};

/** 为工作台底部状态栏生成紧凑运行环境摘要。 */
export function runtimeStatusLabel(status: StartupSnapshot): string {
  if (status.readiness === 'blocked') return '运行能力受限';
  if (status.readiness === 'loading') return '运行环境初始化中';
  const device = status.device?.kind === 'cuda'
    ? `GPU ${status.device.index}`
    : 'CPU';
  return status.degradationReason ? `${device} · 已降级` : `${device} · OCR 就绪`;
}
