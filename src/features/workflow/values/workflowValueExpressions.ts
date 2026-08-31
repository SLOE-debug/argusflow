import type { ValueExpr } from '../model/contracts';
import type { WorkflowNodeData } from '../model/workflowModel';

/** 中央表达式编辑器定位节点内 ValueExpr 的稳定强类型路径。 */
export type ValueExprLocation =
  | { type: 'debug_value' }
  | { type: 'condition_left' }
  | { type: 'condition_right' }
  | { type: 'variable_assignment'; index: number }
  | { type: 'output_binding'; name: string }
  | { type: 'ui_set_value' }
  | { type: 'ui_type_text' }
  | { type: 'navigate_url' }
  | { type: 'format_items' }
  | { type: 'fail_message' }
  | { type: 'component_input'; name: string }
  | {
      type: 'command_field';
      field: 'program' | 'script' | 'working_directory' | 'stdin';
    }
  | { type: 'command_argument'; index: number }
  | { type: 'command_environment'; index: number };

/** 从节点判别联合中读取目标表达式；字段形状变化后返回 null。 */
export function readNodeValueExpr(
  data: WorkflowNodeData,
  location: ValueExprLocation,
): ValueExpr | null {
  switch (location.type) {
    case 'debug_value':
      return data.kind === 'debug' ? data.value : null;
    case 'condition_left':
      return data.kind === 'condition' ? data.left : null;
    case 'condition_right':
      return data.kind === 'condition' ? data.right : null;
    case 'variable_assignment':
      return data.kind === 'variable'
        ? data.assignments[location.index]?.value ?? null
        : null;
    case 'output_binding':
      return data.outputBindings[location.name] ?? null;
    case 'ui_set_value':
      return data.kind === 'ui' && data.operation.type === 'set_value'
        ? data.operation.value
        : null;
    case 'ui_type_text':
      return data.kind === 'ui' && data.operation.type === 'type_text'
        ? data.operation.value
        : null;
    case 'navigate_url':
      return data.kind === 'navigate' ? data.operation.url : null;
    case 'format_items':
      return data.kind === 'format' ? data.operation.items : null;
    case 'fail_message':
      return data.kind === 'fail' ? data.message : null;
    case 'component_input':
      return data.kind === 'component'
        ? data.component.inputs[location.name] ?? null
        : null;
    case 'command_field':
      return data.kind === 'command' ? data.operation[location.field] : null;
    case 'command_argument':
      return data.kind === 'command'
        ? data.operation.arguments[location.index] ?? null
        : null;
    case 'command_environment':
      return data.kind === 'command'
        ? data.operation.environment[location.index]?.value ?? null
        : null;
  }
}

/** 仅在目标仍存在时以不可变方式写回节点表达式。 */
export function updateNodeValueExpr(
  data: WorkflowNodeData,
  location: ValueExprLocation,
  value: ValueExpr,
): WorkflowNodeData {
  switch (location.type) {
    case 'debug_value':
      return data.kind === 'debug' ? { ...data, value, invalid: false } : data;
    case 'condition_left':
      return data.kind === 'condition' ? { ...data, left: value, invalid: false } : data;
    case 'condition_right':
      return data.kind === 'condition' ? { ...data, right: value, invalid: false } : data;
    case 'variable_assignment':
      if (data.kind !== 'variable' || !data.assignments[location.index]) return data;
      return {
        ...data,
        assignments: data.assignments.map((assignment, index) => (
          index === location.index ? { ...assignment, value } : assignment
        )),
        invalid: false,
      };
    case 'output_binding':
      if (!data.outputBindings[location.name]) return data;
      return {
        ...data,
        outputBindings: { ...data.outputBindings, [location.name]: value },
        invalid: false,
      };
    case 'ui_set_value':
      if (data.kind !== 'ui' || data.operation.type !== 'set_value') return data;
      return {
        ...data,
        operation: { ...data.operation, value },
        invalid: false,
      };
    case 'ui_type_text':
      if (data.kind !== 'ui' || data.operation.type !== 'type_text') return data;
      return {
        ...data,
        operation: { ...data.operation, value },
        invalid: false,
      };
    case 'navigate_url':
      return data.kind === 'navigate'
        ? { ...data, operation: { ...data.operation, url: value }, invalid: false }
        : data;
    case 'format_items':
      return data.kind === 'format'
        ? { ...data, operation: { ...data.operation, items: value }, invalid: false }
        : data;
    case 'fail_message':
      return data.kind === 'fail' ? { ...data, message: value, invalid: false } : data;
    case 'component_input':
      if (data.kind !== 'component' || !data.component.inputs[location.name]) return data;
      return {
        ...data,
        component: {
          ...data.component,
          inputs: { ...data.component.inputs, [location.name]: value },
        },
        invalid: false,
      };
    case 'command_field':
      if (data.kind !== 'command' || data.operation[location.field] === null) return data;
      return {
        ...data,
        operation: updateCommandField(data.operation, location.field, value),
        invalid: false,
      };
    case 'command_argument':
      if (data.kind !== 'command' || !data.operation.arguments[location.index]) return data;
      return {
        ...data,
        operation: {
          ...data.operation,
          arguments: data.operation.arguments.map((argument, index) => (
            index === location.index ? value : argument
          )),
        },
        invalid: false,
      };
    case 'command_environment':
      if (data.kind !== 'command' || !data.operation.environment[location.index]) return data;
      return {
        ...data,
        operation: {
          ...data.operation,
          environment: data.operation.environment.map((binding, index) => (
            index === location.index ? { ...binding, value } : binding
          )),
        },
        invalid: false,
      };
  }
}

/** 穷尽更新 Command 的可选单值字段，避免字符串式字段写入扩散到组件。 */
function updateCommandField(
  operation: Extract<WorkflowNodeData, { kind: 'command' }>['operation'],
  field: Extract<ValueExprLocation, { type: 'command_field' }>['field'],
  value: ValueExpr,
): Extract<WorkflowNodeData, { kind: 'command' }>['operation'] {
  switch (field) {
    case 'program':
      return { ...operation, program: value };
    case 'script':
      return { ...operation, script: value };
    case 'working_directory':
      return { ...operation, working_directory: value };
    case 'stdin':
      return { ...operation, stdin: value };
  }
}
