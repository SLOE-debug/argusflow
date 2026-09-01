import type {
  ResourceRef,
  ValueExpr,
  ScopedFlowGraphContract,
  WorkflowInputDefinition,
} from '../model/contracts';

/** 每次运行创建隔离 profile 和随机 CDP 端口的 Chromium 获取契约。 */
export type BrowserSpec = {
  /** Chromium 系浏览器可执行文件的绝对路径。 */
  executable_path: string;
  /** 当前仅支持每次运行创建隔离 CDP 会话。 */
  acquire_mode: 'launch_isolated_cdp';
  /** 等待浏览器公开 CDP page target 的最长毫秒数。 */
  launch_timeout_ms: number;
  /** 工作流结束时关闭本次受管浏览器。 */
  cleanup_policy: 'close_on_workflow_end';
};

/** 在已获取 BrowserSession 上执行的浏览器语义操作。 */
export type BrowserOperation = Readonly<{
  type: 'navigate';
  browser: ResourceRef;
  url: ValueExpr;
}>;

/** 把结构化对象数组格式化为确定文本。 */
export type DelimitedTextFormat = Readonly<{
  items: ValueExpr;
  fields: string[];
  column_separator: string;
  row_separator: string;
  include_header: boolean;
}>;

/** 组件公开的一个具名值输出。 */
export type ComponentValueOutput = Readonly<{
  name: string;
  value: ValueExpr;
}>;

/** 可版本化、可嵌套的流程组件定义。 */
export type FlowComponentDefinition = Readonly<{
  schema_version: 2;
  id: string;
  version: string;
  name: string;
  inputs: WorkflowInputDefinition[];
  outputs: ComponentValueOutput[];
  graph: ScopedFlowGraphContract;
}>;

/** 主画布中的精确版本组件实例 payload。 */
export type ComponentInstance = Readonly<{
  component_id: string;
  component_version: string;
  inputs: Readonly<Record<string, ValueExpr>>;
}>;

/** 执行事件中的一层组件来源。 */
export type ExecutionComponentFrame = Readonly<{
  instance_node_id: string;
  component_id: string;
  component_version: string;
  inner_node_id: string;
}>;

/** 执行事件中的一层 While 激活来源。 */
export type ExecutionLoopFrame = Readonly<{
  container_node_id: string;
  scope_id: string;
  iteration: number;
}>;

/** 组件与 While 共同组成的可判别结构路径。 */
export type ExecutionStructureFrame =
  | Readonly<{ type: 'component' } & ExecutionComponentFrame>
  | Readonly<{ type: 'loop' } & ExecutionLoopFrame>;
