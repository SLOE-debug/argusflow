import { describe, expect, it } from 'vitest';

import type {
  UiExecutionPolicy,
  UiOperation,
  WorkflowResourceCatalog,
} from '../index';
import {
  buildActionInspectorViewModel,
  changeActionLocation,
} from './actionInspectorAdapter';

const EXECUTION: UiExecutionPolicy = {
  target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 200 },
};

describe('actionInspectorAdapter', () => {
  it('uses the application name in the summary and keeps the source node secondary', () => {
    const operation = clickOperation();
    const catalog: WorkflowResourceCatalog = {
      application: [{
        kind: 'application',
        nodeId: 'open-wechat',
        nodeLabel: '打开微信',
        resourceLabel: '微信',
        available: true,
      }],
      browser: [],
    };

    const viewModel = buildActionInspectorViewModel(operation, EXECUTION, catalog);

    expect(viewModel.location).toMatchObject({
      label: '微信',
      sourceLabel: '打开微信',
    });
    expect(viewModel.summary).toBe('在「微信」中找到文字「选择联系人」并单击。');
    expect(viewModel.timeoutSeconds).toBe(5);
    expect(viewModel.retryIntervalMs).toBe(200);
  });

  it('formats a bound target without nested quotation marks', () => {
    const operation = clickOperation();
    if (operation.target.locator.type !== 'query') throw new Error('query target expected');
    const boundOperation: UiOperation = {
      ...operation,
      target: {
        ...operation.target,
        locator: {
          ...operation.target.locator,
          query: {
            ...operation.target.locator.query,
            source: 'text(name = $contact)',
            bindings: {
              contact: {
                type: 'ref',
                source: { type: 'workflow_input', key: '联系人' },
                pointer: '',
              },
            },
          },
        },
      },
    };

    const viewModel = buildActionInspectorViewModel(boundOperation, EXECUTION, {
      application: [{
        kind: 'application',
        nodeId: 'open-wechat',
        nodeLabel: '打开微信',
        resourceLabel: '微信',
        available: true,
      }],
      browser: [],
    });

    expect(viewModel.summary).toBe('在「微信」中找到由流程输入「联系人」指定的文字并单击。');
  });

  it('maps the unified application picker back to the current workflow schema', () => {
    const operation = clickOperation();
    const changed = changeActionLocation(operation, 'application:open-other');

    expect(changed.target.scope).toEqual({
      type: 'application',
      resource: { producer_node_id: 'open-other', output_name: 'session' },
    });
  });
});

/** 创建供适配器用例共享的微信文字点击。 */
function clickOperation(): UiOperation {
  return {
    type: 'click',
    target: {
      scope: {
        type: 'application',
        resource: { producer_node_id: 'open-wechat', output_name: 'session' },
      },
      locator: {
        type: 'query',
        query: {
          language_version: 3,
          source: 'text(name = "选择联系人")',
          bindings: {},
        },
      },
      backend_policy: { allow: [], deny: [], prefer: [] },
    },
  };
}
