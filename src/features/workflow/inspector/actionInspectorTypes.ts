/** 基础表单可以直接表达的目标匹配方式。 */
export type IntentMatchMode =
  | 'exact'
  | 'contains'
  | 'starts_with'
  | 'ends_with'
  | 'regex';

/** 基础表单可以直接表达的控件角色。 */
export type IntentControlRole =
  | 'button'
  | 'textbox'
  | 'checkbox'
  | 'radio'
  | 'link'
  | 'menu_item'
  | 'list_item'
  | 'tab'
  | 'window'
  | 'dialog'
  | 'pane';

/** 查询目标中面向用户展示的值来源。 */
export type IntentTargetValue = Readonly<{
  /** 表单可直接修改字面量；绑定值保持只读，避免破坏参数契约。 */
  source: 'literal' | 'binding';
  /** 字面量文本或参数绑定的人类可读说明。 */
  text: string;
  /** 绑定参数名只在 source 为 binding 时存在。 */
  bindingName?: string;
}>;

/** 从现有 AQL 适配出的语义目标判别联合。 */
export type QueryTargetIntent =
  | Readonly<{
      type: 'text';
      value: IntentTargetValue;
      match: IntentMatchMode;
      editable: boolean;
      hasMoreConditions: boolean;
    }>
  | Readonly<{
      type: 'control';
      role: IntentControlRole;
      value: IntentTargetValue;
      match: IntentMatchMode;
      editable: boolean;
      hasMoreConditions: boolean;
    }>
  | Readonly<{
      type: 'web';
      selector: string;
      editable: boolean;
    }>
  | Readonly<{
      type: 'advanced';
      description: string;
    }>;

/** Intent 面板当前选择的应用或窗口。 */
export type ActionLocationViewModel = Readonly<{
  /** 统一选择器使用的稳定值。 */
  value: 'current' | `application:${string}` | `browser:${string}`;
  /** 首先展示真实应用/窗口名称。 */
  label: string;
  /** 资源由上游节点产生时展示的弱提示。 */
  sourceLabel: string | null;
  /** 已失效或不支配当前节点的资源原因。 */
  unavailableReason: string | null;
}>;

/** Action 主面板可以直接编辑的完整目标联合。 */
export type ActionTargetIntent = QueryTargetIntent
  | Readonly<{ type: 'coordinate'; x: number; y: number }>
  | Readonly<{ type: 'focus' }>;

/** 当前环境目标检查的结构化状态，为后续 P1 match count 保留稳定入口。 */
export type ActionTargetStatus =
  | Readonly<{ type: 'unchecked' }>
  | Readonly<{ type: 'configured'; message: string }>
  | Readonly<{ type: 'invalid'; message: string }>
  | Readonly<{ type: 'matched'; count: number }>;

/** 现有 workflow engine 字段映射出的任务意图视图。 */
export type ActionInspectorViewModel = Readonly<{
  summary: string;
  actionLabel: string;
  location: ActionLocationViewModel;
  target: ActionTargetIntent;
  targetStatus: ActionTargetStatus;
  waitForTarget: boolean;
  timeoutSeconds: number;
  retryIntervalMs: number;
  locatorEngineLabel: string;
  actionEngineLabel: string;
}>;

/** 控件角色到普通用户可读名称的稳定词典。 */
export const INTENT_CONTROL_ROLE_LABELS: Readonly<Record<IntentControlRole, string>> = {
  button: '按钮',
  textbox: '输入框',
  checkbox: '复选框',
  radio: '单选项',
  link: '链接',
  menu_item: '菜单项',
  list_item: '列表项',
  tab: '标签页',
  window: '窗口',
  dialog: '对话框',
  pane: '区域',
};

/** 严格识别基础表单支持的控件角色。 */
export function isIntentControlRole(role: string): role is IntentControlRole {
  return Object.hasOwn(INTENT_CONTROL_ROLE_LABELS, role);
}
