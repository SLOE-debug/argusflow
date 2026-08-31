import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { WorkflowCanvasNode } from '../../../../features/workflow';
import { buildWorkflowSymbolRegistry } from '../../../../features/workflow';
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
  it('selects a published output through the unified picker', () => {
    const onChange = vi.fn();
    const upstream = upstreamCommand();
    const symbols = buildWorkflowSymbolRegistry({
      inputs: [],
      variables: {},
      nodes: [upstream],
      edges: [],
    });
    render(
      <ValueExprEditorProvider
        value={{ symbols, onOpenExpression: vi.fn() }}
      >
        <ValueExprFields
          value={{ type: 'literal', value: '' }}
          onChange={onChange}
        />
      </ValueExprEditorProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: '输入值：选择工作流值' }));
    fireEvent.click(screen.getByRole('option', { name: /写入文件 · output（自定义）/ }));
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
          symbols: { inputs: [], variables: [], nodeOutputs: [] },
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

    fireEvent.click(screen.getByRole('button', { name: '编辑' }));
    expect(onOpenExpression).toHaveBeenCalledWith({ type: 'debug_value' });
  });
});
