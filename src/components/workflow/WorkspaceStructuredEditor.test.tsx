import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { WorkflowCanvasNode } from '../../features/workflow/workflowModel';
import { WorkspaceStructuredEditor } from './WorkspaceStructuredEditor';

/** 创建固定文本 PowerShell 节点，验证按 ID 写回的独立编辑边界。 */
function createCommandNode(): WorkflowCanvasNode {
  return {
    id: 'command-1',
    kind: 'command',
    position: { x: 0, y: 0 },
    size: { width: 164, height: 52 },
    data: {
      kind: 'command',
      label: '写入文件',
      operation: {
        runner: 'power_shell',
        program: null,
        arguments: [],
        script: { type: 'literal', value: 'echo first' },
        working_directory: null,
        environment: [],
        stdin: null,
        timeout_ms: 30_000,
        accepted_exit_codes: [0],
        max_stdout_bytes: 1_048_576,
        max_stderr_bytes: 1_048_576,
      },
    },
  };
}

describe('WorkspaceStructuredEditor', () => {
  it('writes a script through its target node ID instead of current selection', () => {
    const node = createCommandNode();
    const onUpdateNode = vi.fn();
    render(
      <WorkspaceStructuredEditor
        target={{ type: 'command_script', nodeId: node.id }}
        nodes={[node]}
        onUpdateNode={onUpdateNode}
      />,
    );

    fireEvent.change(screen.getByRole('textbox', { name: '脚本内容' }), {
      target: { value: 'echo first\necho second' },
    });

    expect(onUpdateNode).toHaveBeenCalledWith(node.id, expect.any(Function));
    const updater = onUpdateNode.mock.calls[0]?.[1] as (current: typeof node.data) => typeof node.data;
    const updated = updater(node.data);
    expect(updated).toMatchObject({
      kind: 'command',
      operation: {
        script: { type: 'literal', value: 'echo first\necho second' },
      },
    });
  });
});
