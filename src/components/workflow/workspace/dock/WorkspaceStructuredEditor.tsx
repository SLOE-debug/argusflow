import type {
  AqlQuery,
  ValidationReport,
} from '../../../../features/workflow';
import { AqlEditor } from '../../../../features/aql-editor/view/AqlEditor';
import { changeTargetLocator } from '../../../../features/workflow';
import {
  readNodeValueExpr,
  updateNodeValueExpr,
} from '../../../../features/workflow';
import type {
  WorkflowCanvasNode,
  WorkflowNodeUpdater,
} from '../../../../features/workflow';
import { CommandScriptEditor } from '../../inspector/node-fields/CommandScriptEditor';
import { ExpressionEditor } from '../../inspector/node-fields/ExpressionEditor';
import type { StructuredEditorTarget } from './structuredEditorTarget';

type WorkspaceStructuredEditorProps = Readonly<{
  /** 当前打开的结构化文档目标。 */
  target: StructuredEditorTarget;
  /** 实时工作流节点集合。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 最近一次 Runtime 校验结果，用于表达式编译诊断。 */
  report?: ValidationReport | null;
  /** 按目标节点写回，不依赖当前画布选择。 */
  onUpdateNode: (nodeId: string, updater: WorkflowNodeUpdater) => void;
}>;

/** 从实时工作流状态解析并承载唯一的 Workspace Monaco 文档。 */
export function WorkspaceStructuredEditor({
  target,
  nodes,
  report = null,
  onUpdateNode,
}: WorkspaceStructuredEditorProps) {
  const node = nodes.find((candidate) => candidate.id === target.nodeId);
  if (!node) {
    return <UnavailableEditor message="这个节点已不存在，请关闭此编辑器。" />;
  }

  switch (target.type) {
    case 'aql': {
      const operation = node.data.kind === 'ui' ? node.data.operation : null;
      if (!operation || operation.target.locator.type !== 'query') {
        return <UnavailableEditor message="此节点已改用其他查找方式，请关闭此编辑器。" />;
      }
      const locator = operation.target.locator;
      return (
        <AqlEditor
          query={locator.query}
          target={operation.target}
          modelUri={`inmemory://argusflow/workflow/${encodeURIComponent(node.id)}/locator-aql`}
          onChange={(query) => onUpdateNode(node.id, (current) => (
            updateAqlQuery(current, query)
          ))}
        />
      );
    }
    case 'command_script': {
      const operation = node.data.kind === 'command' ? node.data.operation : null;
      const script = operation?.script;
      if (
        !operation
        || operation.runner === 'direct'
        || !script
        || script.type !== 'literal'
        || typeof script.value !== 'string'
      ) {
        return <UnavailableEditor message="此节点已改用其他执行方式，请关闭此编辑器。" />;
      }
      return (
        <CommandScriptEditor
          runner={operation.runner}
          nodeId={node.id}
          source={script.value}
          onChange={(source) => onUpdateNode(node.id, (current) => (
            updateCommandScript(current, source)
          ))}
        />
      );
    }
    case 'expression': {
      const expression = readNodeValueExpr(node.data, target.location);
      if (expression?.type !== 'expression') {
        return <UnavailableEditor message="此字段已改为其他数据来源，请关闭此编辑器。" />;
      }
      /** location 进入 URI，确保同一节点的不同表达式保留各自 Monaco 模型。 */
      const locationKey = encodeURIComponent(JSON.stringify(target.location));
      const compileError = report?.issues.find((issue) => (
        issue.code === 'invalid_expression' && issue.node_id === node.id
      ))?.message ?? null;
      return (
        <ExpressionEditor
          modelUri={`inmemory://argusflow/workflow/${encodeURIComponent(node.id)}/expression/${locationKey}`}
          source={expression.source}
          nodes={nodes}
          compileError={compileError}
          onChange={(source) => onUpdateNode(node.id, (current) => (
            updateNodeValueExpr(current, target.location, { type: 'expression', source })
          ))}
        />
      );
    }
  }
}

/** 仅在节点仍满足目标判别条件时更新 AQL。 */
function updateAqlQuery(
  current: WorkflowCanvasNode['data'],
  query: AqlQuery,
): WorkflowCanvasNode['data'] {
  if (current.kind !== 'ui' || current.operation.target.locator.type !== 'query') {
    return current;
  }
  return {
    ...current,
    operation: changeTargetLocator(
      current.operation,
      { ...current.operation.target.locator, query },
    ),
    invalid: false,
  };
}

/** 仅在节点仍为固定文本 shell 命令时更新脚本。 */
function updateCommandScript(
  current: WorkflowCanvasNode['data'],
  source: string,
): WorkflowCanvasNode['data'] {
  if (
    current.kind !== 'command'
    || current.operation.runner === 'direct'
    || current.operation.script?.type !== 'literal'
  ) {
    return current;
  }
  return {
    ...current,
    operation: {
      ...current.operation,
      script: { type: 'literal', value: source },
    },
    invalid: false,
  };
}

/** 文档目标失效时保留非模态工作区并给出明确退出路径。 */
function UnavailableEditor({ message }: Readonly<{ message: string }>) {
  return (
    <div className="flex h-full items-center justify-center bg-white p-6">
      <p className="rounded-lg border border-dashed border-amber-300 bg-amber-50 px-6 py-4 text-[12px] text-amber-700">
        {message}
      </p>
    </div>
  );
}
