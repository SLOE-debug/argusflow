import type { AqlQuery } from '../../features/workflow/contracts';
import { AqlEditor } from '../../features/aql-editor/view/AqlEditor';
import { changeTargetLocator } from '../../features/workflow/workflowAction';
import type {
  WorkflowCanvasNode,
  WorkflowNodeUpdater,
} from '../../features/workflow/workflowModel';
import { CommandScriptEditor } from './CommandScriptEditor';
import type { StructuredEditorTarget } from './structuredEditorTarget';

type WorkspaceStructuredEditorProps = Readonly<{
  /** 当前打开的结构化文档目标。 */
  target: StructuredEditorTarget;
  /** 实时工作流节点集合。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 按目标节点写回，不依赖当前画布选择。 */
  onUpdateNode: (nodeId: string, updater: WorkflowNodeUpdater) => void;
}>;

/** 从实时工作流状态解析并承载唯一的 Workspace Monaco 文档。 */
export function WorkspaceStructuredEditor({
  target,
  nodes,
  onUpdateNode,
}: WorkspaceStructuredEditorProps) {
  const node = nodes.find((candidate) => candidate.id === target.nodeId);
  if (!node) {
    return <UnavailableEditor message="所属节点已不存在，请关闭此编辑器。" />;
  }

  switch (target.type) {
    case 'aql': {
      if (node.data.kind !== 'ui' || node.data.operation.target.locator.type !== 'query') {
        return <UnavailableEditor message="该节点已不再使用 AQL 查找规则。" />;
      }
      const operation = node.data.operation;
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
        return <UnavailableEditor message="该节点已不再使用固定文本脚本。" />;
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
