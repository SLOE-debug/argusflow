/** 任务列表中的有限运行状态。 */
export type WorkflowTaskStatus = 'running' | 'success' | 'failed' | 'paused';

/** 工作台任务表格使用的只读展示模型。 */
export type WorkflowTaskRow = Readonly<{
  /** 任务唯一标识。 */
  id: string;
  /** 面向用户的任务名称。 */
  name: string;
  /** 当前运行状态。 */
  status: WorkflowTaskStatus;
  /** 最近一次执行人。 */
  operator: string;
  /** 最近一次启动时间。 */
  lastRun: string;
  /** 最近一次耗时；从未运行时为破折号。 */
  duration: string;
  /** 成功次数。 */
  succeeded: number | null;
  /** 总执行次数；持续任务使用 infinity。 */
  total: number | typeof Infinity | null;
}>;

/** 参考图中的高密度任务列表数据。 */
export const WORKFLOW_TASK_ROWS = [
  {
    id: 'daily-sync',
    name: '每日数据同步',
    status: 'running',
    operator: '张三',
    lastRun: '2024-05-20 10:31:45',
    duration: '00:02:18',
    succeeded: 12,
    total: Infinity,
  },
  {
    id: 'user-analysis',
    name: '用户行为分析',
    status: 'success',
    operator: '李四',
    lastRun: '2024-05-20 09:15:22',
    duration: '00:08:42',
    succeeded: 24,
    total: 24,
  },
  {
    id: 'order-cleaning',
    name: '订单清洗任务',
    status: 'failed',
    operator: '王五',
    lastRun: '2024-05-20 08:45:11',
    duration: '00:01:05',
    succeeded: 3,
    total: 24,
  },
  {
    id: 'backup-archive',
    name: '数据备份归档',
    status: 'paused',
    operator: '赵六',
    lastRun: '2024-05-19 23:59:00',
    duration: '—',
    succeeded: null,
    total: null,
  },
  {
    id: 'realtime-push',
    name: '实时告警推送',
    status: 'success',
    operator: '张三',
    lastRun: '2024-05-19 22:10:33',
    duration: '00:00:45',
    succeeded: 128,
    total: 128,
  },
] as const satisfies ReadonlyArray<WorkflowTaskRow>;
