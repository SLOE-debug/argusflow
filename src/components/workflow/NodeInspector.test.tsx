import { act, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { createFlowStore } from '../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';
import { DEFAULT_WORKFLOW_PERMISSIONS } from '../../features/workflow/defaultWorkflowTemplate';
import { NodeInspector } from './NodeInspector';

describe('NodeInspector', () => {
  it('uses one property panel and follows the current selection', () => {
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>({
      nodes: [{
        id: 'log-1',
        kind: 'log',
        position: { x: 10, y: 20 },
        size: { width: 142, height: 52 },
        data: { kind: 'log', label: '日志', outputBindings: {}, message: '测试' },
      }],
    });
    render(
      <NodeInspector
        store={store}
        workflowName="测试流程"
        variablesDraft="{}"
        variablesError={null}
        inputDefinitionsDraft="[]"
        inputDefinitionsError={null}
        runInputValuesDraft="{}"
        runInputValuesError={null}
        permissions={DEFAULT_WORKFLOW_PERMISSIONS}
        onNameChange={vi.fn()}
        onVariablesChange={vi.fn()}
        onInputDefinitionsChange={vi.fn()}
        onRunInputValuesChange={vi.fn()}
        onPermissionsChange={vi.fn()}
        onUpdateNode={vi.fn()}
        onUpdateEdgeBranch={vi.fn()}
        onOpenStructuredEditor={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByRole('heading', { name: '属性' })).toBeVisible();
    expect(screen.getByDisplayValue('测试流程')).toBeVisible();
    expect(screen.queryByRole('button', { name: '流程设置' })).not.toBeInTheDocument();

    act(() => store.getState().selectNodes(['log-1']));
    expect(screen.getByText('节点', { selector: 'span' })).toBeVisible();
    expect(screen.getByDisplayValue('log-1')).toBeVisible();
    expect(screen.queryByDisplayValue('测试流程')).not.toBeInTheDocument();
  });
});
