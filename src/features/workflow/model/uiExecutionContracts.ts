import type { VisualQueryExpr } from './visual';

/** 当前 UI 节点自己的目标就绪等待模式。 */
export type TargetWaitMode = 'none' | 'bounded';

/** 只对当前 operation 的 `TargetNotFound` 生效的等待预算。 */
export type TargetWaitPolicy = Readonly<{
  mode: TargetWaitMode;
  timeout_ms: number;
  poll_interval_ms: number;
}>;

/** UI 节点与动作语义分离的执行策略。 */
export type UiPostcondition =
  | {
      /** 要求动作后同一区域内的目标文字实例数量严格增加。 */
      type: 'new_text';
      query: VisualQueryExpr;
      /** 动作前后都必须唯一命中且保持在原位置的上下文。 */
      stable_context: ReadonlyArray<VisualQueryExpr>;
    }
  | {
      /** 要求动作后的新鲜画面中唯一存在目标文字。 */
      type: 'text_present';
      query: VisualQueryExpr;
    };

/** UI 节点自己的目标等待与动作后置条件契约。 */
export type UiExecutionPolicy = Readonly<{
  /** 当前 operation 找不到目标时使用的共享等待预算。 */
  target_wait: TargetWaitPolicy;
  /** 动作完成后观察视觉后置条件的独立等待预算。 */
  postcondition_wait: TargetWaitPolicy;
  /** 高风险输入动作完成后必须满足的视觉新事实。 */
  postcondition: UiPostcondition | null;
}>;
