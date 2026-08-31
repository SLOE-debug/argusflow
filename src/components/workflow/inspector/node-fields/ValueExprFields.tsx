import {
  createContext,
  useContext,
  type ReactNode,
} from 'react';

import type {
  ValueExpr,
  ValueExprLocation,
  WorkflowSymbolRegistry,
} from '../../../../features/workflow';
import {
  ValueField,
  type ValueLiteralEditorProps,
  type ValueLiteralPresentation,
} from '../../value-editor/ValueField';

/** ValueExpr 常量与业务语义解耦后的展示方式。 */
export type LiteralPresentation = ValueLiteralPresentation;

/** 业务专属字符串常量编辑器获得的最小受控契约。 */
export type LiteralEditorProps = ValueLiteralEditorProps;

type ValueExprFieldsProps = Readonly<{
  /** 当前完整值表达式。 */
  value: ValueExpr;
  /** 写回字段完整的新表达式。 */
  onChange: (value: ValueExpr) => void;
  /** 常量输入框使用的可访问名称。 */
  literalLabel?: string;
  /** 文本消费边界使用字符串，JSON 数据边界使用完整 JSON 编辑。 */
  literalMode?: 'text' | 'json';
  /** 字符串常量的业务专属展示方式。 */
  literalPresentation?: LiteralPresentation;
  /** 中央 Workspace 定位当前表达式字段所需的稳定路径。 */
  expressionLocation?: ValueExprLocation;
}>;

export type ValueExprEditorContextValue = Readonly<{
  /** 请求中央 Workspace 打开指定表达式。 */
  onOpenExpression: (location: ValueExprLocation) => void;
  /** 从当前工作流快照派生的统一值目录。 */
  symbols?: WorkflowSymbolRegistry;
}>;

const ValueExprEditorContext = createContext<ValueExprEditorContextValue | null>(null);

/** 读取当前节点字段共享的工作流值目录与表达式路由。 */
export function useValueExprEditorContext(): ValueExprEditorContextValue | null {
  return useContext(ValueExprEditorContext);
}

/** 为一个节点的全部 ValueExpr 编辑器提供统一值目录与 Workspace 路由。 */
export function ValueExprEditorProvider({
  value,
  children,
}: Readonly<{ value: ValueExprEditorContextValue; children: ReactNode }>) {
  return (
    <ValueExprEditorContext.Provider value={value}>
      {children}
    </ValueExprEditorContext.Provider>
  );
}

/** 统一编辑常量、结构化值引用与受限 Rhai 表达式。 */
export function ValueExprFields({
  value,
  onChange,
  literalLabel = '输入值',
  literalMode = 'text',
  literalPresentation = { type: 'single_line' },
  expressionLocation,
}: ValueExprFieldsProps) {
  const editorContext = useContext(ValueExprEditorContext);
  return (
    <ValueField
      label={literalLabel}
      value={value}
      symbols={editorContext?.symbols}
      literalMode={literalMode}
      literalPresentation={literalPresentation}
      allowExpression={Boolean(editorContext && expressionLocation)}
      expressionLocation={expressionLocation}
      onOpenExpression={editorContext?.onOpenExpression}
      onChange={onChange}
    />
  );
}
