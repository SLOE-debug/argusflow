/** 与 Rust 后端交换的完整工作流定义；schema_version 固定为当前契约版本 1。 */
export type WorkflowDefinition = {
  /** 用于后端按版本解析节点和边的契约版本号。 */
  schema_version: 1;
  /** 工作流在当前编辑会话中的稳定标识。 */
  id: string;
  /** 面向用户展示和保存的工作流名称。 */
  name: string;
  /** 按画布节点顺序序列化后的节点集合。 */
  nodes: WorkflowNodeContract[];
  /** 描述节点连接关系的有向边集合。 */
  edges: WorkflowEdgeContract[];
};

/** 后端可执行节点的通用字段与具体节点类型联合。 */
export type WorkflowNodeContract = {
  /** 在画布和执行事件中关联节点的唯一标识。 */
  id: string;
  /** 画布坐标，单位为 React Flow 的像素逻辑坐标。 */
  position: Position;
} & WorkflowNodeKind;

/** 工作流节点在编辑画布中的二维位置。 */
export type Position = {
  /** 水平坐标，向右为正。 */
  x: number;
  /** 垂直坐标，向下为正。 */
  y: number;
};

/** 当前后端支持的节点行为；不同分支携带不同的执行参数。 */
export type WorkflowNodeKind =
  | { type: 'start' }
  | {
      type: 'log';
      /** 节点运行时写入执行日志的文本。 */
      message: string;
    }
  | {
      type: 'delay';
      /** 节点暂停时长，单位为毫秒。 */
      milliseconds: number;
    }
  | {
      type: 'action';
      /** 待执行的桌面或浏览器自动化动作。 */
      action: AutomationAction;
    }
  | { type: 'end' };

/** 可由工作流触发的自动化动作及其目标选择器。 */
export type AutomationAction =
  | {
      type: 'click';
      /** 点击动作定位到的目标元素。 */
      target: Selector;
    }
  | {
      type: 'set_value';
      /** 写入值动作定位到的目标元素。 */
      target: Selector;
      /** 要写入目标控件的文本值。 */
      value: string;
    };

/** 描述桌面控件、浏览器元素、视觉文本或屏幕坐标的定位方式。 */
export type Selector =
  | {
      type: 'native';
      /** Windows 原生控件名称；未知时为 null。 */
      name: string | null;
      /** 原生控件自动化 ID；未知时为 null。 */
      automation_id: string | null;
      /** 原生控件类型；未知时为 null。 */
      control_type: string | null;
    }
  | {
      type: 'browser';
      /** 浏览器 DOM 元素的 CSS 选择器。 */
      css: string;
    }
  | {
      type: 'visual_text';
      /** 屏幕上待匹配的可见文字。 */
      text: string;
      /** 是否要求文字完全匹配。 */
      exact: boolean;
    }
  | {
      type: 'coordinate';
      /** 屏幕水平坐标，单位为像素。 */
      x: number;
      /** 屏幕垂直坐标，单位为像素。 */
      y: number;
    };

/** 工作流中两个节点之间的有向连接。 */
export type WorkflowEdgeContract = {
  /** 边的唯一标识，用于校验结果和画布选中状态关联。 */
  id: string;
  /** 起始节点 ID。 */
  source: string;
  /** 目标节点 ID。 */
  target: string;
};

/** 后端返回的单条结构校验问题；节点或边可能为空，表示工作流级问题。 */
export type ValidationIssue = {
  /** 稳定的机器可读问题代码。 */
  code: string;
  /** 面向用户展示的问题描述。 */
  message: string;
  /** 相关节点 ID；无关联节点时为 null。 */
  node_id: string | null;
  /** 相关边 ID；无关联边时为 null。 */
  edge_id: string | null;
};

/** 一次工作流校验的结果及其问题明细。 */
export type ValidationReport = {
  /** 为 true 表示可提交执行；为 false 时应优先展示 issues。 */
  valid: boolean;
  /** 校验失败时的全部问题，而非仅第一条问题。 */
  issues: ValidationIssue[];
};

/** 启动工作流后后端返回的运行实例信息。 */
export type RunStarted = {
  /** 后续执行事件用于关联本次运行的 ID。 */
  run_id: string;
};

/** 执行事件的类型，覆盖工作流、节点和日志三个层级。 */
export type ExecutionEventKind =
  | 'workflow_started'
  | 'node_started'
  | 'log'
  | 'node_succeeded'
  | 'node_failed'
  | 'workflow_completed'
  | 'workflow_failed';

/** 后端推送给前端的有序执行事件。 */
export type ExecutionEvent = {
  /** 本次运行的唯一标识。 */
  run_id: string;
  /** 产生事件的工作流 ID。 */
  workflow_id: string;
  /** 从 0 开始递增的运行内事件序号。 */
  sequence: number;
  /** 相关节点 ID；工作流级事件没有节点时为 null。 */
  node_id: string | null;
  /** 事件分类，用于更新节点状态和渲染日志色调。 */
  kind: ExecutionEventKind;
  /** 可选的后端消息；无消息时为 null。 */
  message: string | null;
};

/** Tauri 命令失败时统一提供给界面的错误结构。 */
export type CommandError = {
  /** 稳定的机器可读错误代码。 */
  code: string;
  /** 面向用户展示的错误消息。 */
  message: string;
  /** 若错误来自校验，则携带可定位到节点或边的问题。 */
  issues: ValidationIssue[];
};
