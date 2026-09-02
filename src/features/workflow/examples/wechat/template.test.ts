import { describe, expect, it } from 'vitest';

import { FLOW_COMPONENT_CATALOG } from '../../components/componentCatalog';
import { toWorkflowDefinition } from '../../model/workflowModel';
import {
  WECHAT_CONTACT_RESULT_SELECTOR,
  WECHAT_CONVERSATION_READY_QUERY,
  WECHAT_MESSAGE_SENT_QUERY,
} from './automation';
import {
  WECHAT_WORKFLOW_EDGES,
  WECHAT_WORKFLOW_INPUTS,
  WECHAT_WORKFLOW_NAME,
  WECHAT_WORKFLOW_NODES,
  WECHAT_WORKFLOW_PERMISSIONS,
  WECHAT_WORKFLOW_VARIABLES,
  WECHAT_WORKFLOW_DOCUMENTS,
  WECHAT_ROOT_SCOPE_ID,
  WECHAT_SCOPE_METADATA,
} from './template';

describe('微信联系人消息示例工作流', () => {
  it('在默认画布展开搜索、选择、发送和结果检查步骤', () => {
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      WECHAT_WORKFLOW_NAME,
      WECHAT_WORKFLOW_INPUTS,
      WECHAT_WORKFLOW_VARIABLES,
      WECHAT_WORKFLOW_PERMISSIONS,
      WECHAT_ROOT_SCOPE_ID,
      WECHAT_WORKFLOW_DOCUMENTS,
      WECHAT_SCOPE_METADATA,
    );
    /** 用 ID 定位关键节点，避免测试依赖数组中的视觉排列。 */
    const nodesById = new Map(workflow.graph.scopes
      .flatMap((scope) => scope.nodes)
      .map((node) => [node.id, node]));

    expect(FLOW_COMPONENT_CATALOG).toEqual([]);
    expect(WECHAT_CONTACT_RESULT_SELECTOR).toContain(
      'text(name matches /\\b(?:联系人|最常使用|最近常用|功能|群聊)\\b/)',
    );
    expect(WECHAT_CONTACT_RESULT_SELECTOR).toContain(
      'target = text(name contains $contact_name)',
    );
    expect(WECHAT_CONVERSATION_READY_QUERY).toContain(
      'anchor = viewport_edge(side = top)',
    );
    expect(WECHAT_CONVERSATION_READY_QUERY).toContain(
      'target = text(name contains $contact_name)',
    );
    expect(nodesById.get('select_contact')?.payload).toEqual(expect.objectContaining({
      operation: expect.objectContaining({
        type: 'click',
        target: expect.objectContaining({
          locator: expect.objectContaining({
            query: expect.objectContaining({
              language_version: 3,
              source: WECHAT_CONTACT_RESULT_SELECTOR,
            }),
          }),
        }),
      }),
    }));
    expect(nodesById.get('check_send_result')?.payload).toEqual(expect.objectContaining({
      observation: expect.objectContaining({
        query: expect.objectContaining({
          language_version: 3,
          source: WECHAT_MESSAGE_SENT_QUERY,
        }),
      }),
    }));
    expect(nodesById.get('check_conversation')?.payload).toEqual(expect.objectContaining({
      observation: expect.objectContaining({
        query: expect.objectContaining({
          source: WECHAT_CONVERSATION_READY_QUERY,
        }),
      }),
    }));
    const edges = workflow.graph.scopes.flatMap((scope) => scope.edges);
    expect(edges).toEqual(expect.arrayContaining([
      expect.objectContaining({
        source: 'check_search',
        target: 'scope_wait_for_search_complete',
        branch: 'true',
      }),
      expect.objectContaining({
        source: 'check_conversation',
        target: 'scope_wait_for_conversation_complete',
        branch: 'true',
      }),
      expect.objectContaining({
        source: 'check_send_result',
        target: 'scope_wait_for_send_result_complete',
        branch: 'true',
      }),
      expect.objectContaining({
        source: 'check_send_result',
        target: 'scope_wait_for_send_result_continue',
        branch: 'false',
      }),
      expect.objectContaining({
        source: 'wait_for_send_result',
        target: 'send_result_unknown',
        branch: 'exhausted',
      }),
    ]));
  });
});
