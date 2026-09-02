import type { WorkflowNodeData } from '../../../features/workflow';

/** 节点类型的稳定中文名称。 */
export const NODE_KIND_LABELS: Readonly<Record<WorkflowNodeData['kind'], string>> = {
  start: '开始',
  log: '记录日志',
  debug: '查看结果',
  delay: '等待一段时间',
  condition: '条件判断',
  observe: '检查界面',
  loop: '重复执行',
  loopEntry: '每轮开始',
  loopContinue: '继续下一轮',
  loopComplete: '完成循环',
  variable: '设置变量',
  application: '打开应用',
  browser: '打开浏览器',
  navigate: '打开网页',
  ui: '操作界面',
  command: '执行命令',
  format: '整理文本',
  component: '组合步骤',
  fail: '停止并报错',
  end: '结束',
};

/** 每类节点主设置区使用任务语言，而非统一的空泛“设置”。 */
export const NODE_SETTINGS_TITLES: Readonly<Record<WorkflowNodeData['kind'], string>> = {
  start: '开始方式',
  log: '记录什么',
  debug: '查看什么',
  delay: '等待多久',
  condition: '判断什么',
  observe: '检查什么',
  loop: '怎样重复',
  loopEntry: '循环位置',
  loopContinue: '循环位置',
  loopComplete: '循环位置',
  variable: '保存什么',
  application: '打开什么',
  browser: '打开什么',
  navigate: '前往哪里',
  ui: '做什么',
  command: '执行什么',
  format: '怎样整理',
  component: '使用哪个步骤',
  fail: '为什么停止',
  end: '结束方式',
};

/** 为通用节点生成一眼可确认用途的简短摘要。 */
export function formatNodeInspectorSummary(data: WorkflowNodeData): string {
  switch (data.kind) {
    case 'start':
      return '流程从这里开始。';
    case 'log':
      return '把指定内容写入运行日志。';
    case 'debug':
      return '在运行记录中查看指定值。';
    case 'delay':
      return `等待 ${formatDuration(data.milliseconds)} 后继续。`;
    case 'condition':
      return '根据条件选择后续分支。';
    case 'observe':
      return '检查当前界面并返回结果。';
    case 'loop':
      return '按次数或时间预算重复执行一组步骤。';
    case 'loopEntry':
      return 'While 的每一轮从这里开始。';
    case 'loopContinue':
      return '检查预算后开始下一轮。';
    case 'loopComplete':
      return '条件成立后离开 While。';
    case 'variable':
      return '保存流程中需要重复使用的数据。';
    case 'application':
      return '打开应用或连接已有窗口。';
    case 'browser':
      return '打开一个独立浏览器。';
    case 'navigate':
      return '在浏览器中打开指定网址。';
    case 'ui':
      return '在界面中执行操作。';
    case 'command':
      return '运行程序或系统命令。';
    case 'format':
      return '把数据整理成可读文本。';
    case 'component':
      return '运行一个版本锁定的组合步骤。';
    case 'fail':
      return '停止流程并说明失败原因。';
    case 'end':
      return '运行到这里，流程结束。';
  }
}

/** 毫秒配置在摘要中使用更符合阅读习惯的单位。 */
function formatDuration(milliseconds: number): string {
  return milliseconds >= 1_000
    ? `${milliseconds / 1_000} 秒`
    : `${milliseconds} 毫秒`;
}
