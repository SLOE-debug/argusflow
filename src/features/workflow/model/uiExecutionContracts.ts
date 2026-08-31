/** 当前 UI 节点自己的目标就绪等待模式。 */
export type TargetWaitMode = 'none' | 'bounded';

/** 只对当前 operation 的 `TargetNotFound` 生效的等待预算。 */
export type TargetWaitPolicy = Readonly<{
  mode: TargetWaitMode;
  timeout_ms: number;
  poll_interval_ms: number;
}>;

/** UI 写操作自己的目标等待契约。 */
export type UiExecutionPolicy = Readonly<{
  /** 当前 operation 找不到目标时使用的共享等待预算。 */
  target_wait: TargetWaitPolicy;
}>;
