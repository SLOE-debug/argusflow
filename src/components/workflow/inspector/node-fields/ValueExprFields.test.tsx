import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { WorkflowCanvasNode } from '../../../../features/workflow';
import {
  ValueExprEditorProvider,
  ValueExprFields,
} from './ValueExprFields';

/** 创建一个公开原生与自定义输出的上游命令节点。 */
function upstreamCommand(): WorkflowCanvasNode {
  return {
    id: 'write-file',
    kind: 'command',
    position: { x: 0, y: 0 },
    size: { width: 164, height: 52 },
    data: {
      kind: 'command',
      label: '写入文件',
      outputBindings: {
        output: { type: 'expression', source: 'result.stdout' },
      },
      operation: {
        runner: 'power_shell',
        program: null,
        arguments: [],
        script: { type: 'literal', value: 'echo path' },
        working_directory: null,
        environment: [],
        stdin: null,
        timeout_ms: 30_000,
        accepted_exit_codes: [0],
        max_stdout_bytes: 1_024,
        max_stderr_bytes: 1_024,
      },
    },
  };
}

describe('ValueExprFields', () => {
  it('selects upstream nodes and known outputs without a producer ID textbox', () => {
    const onChange = vi.fn();
    render(
      <ValueExprEditorProvider
        value={{
          upstreamNodes: [upstreamCommand()],
          workflowInputs: [],
          variableNames: [],
          onOpenExpression: vi.fn(),
        }}
      >
        <ValueExprFields
          value={{
            type: 'ref',
            source: { type: 'node', node_id: 'write-file' },
            pointer: '',
          }}
          onChange={onChange}
        />
      </ValueExprEditorProvider>,
    );

    expect(screen.getByRole('combobox', { name: '上游节点' })).toHaveTextContent('写入文件');
    expect(screen.queryByRole('textbox', { name: /生产节点/ })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('combobox', { name: '要读取的内容' }));
    fireEvent.click(screen.getByRole('option', { name: 'output（自定义）' }));
    expect(onChange).toHaveBeenCalledWith({
      type: 'ref',
      source: { type: 'node', node_id: 'write-file' },
      pointer: '/output',
    });
  });

  it('routes expression editing through the central workspace location', () => {
    const onOpenExpression = vi.fn();
    render(
      <ValueExprEditorProvider
        value={{
          upstreamNodes: [],
          workflowInputs: [],
          variableNames: [],
          onOpenExpression,
        }}
      >
        <ValueExprFields
          value={{ type: 'expression', source: 'vars.count + 1' }}
          expressionLocation={{ type: 'debug_value' }}
          onChange={vi.fn()}
        />
      </ValueExprEditorProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: '编辑表达式' }));
    expect(onOpenExpression).toHaveBeenCalledWith({ type: 'debug_value' });
  });
});
