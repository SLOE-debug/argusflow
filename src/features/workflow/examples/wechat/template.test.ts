import { describe, expect, it } from 'vitest';

import { FLOW_COMPONENT_CATALOG } from '../../components/componentCatalog';
import { toWorkflowDefinition } from '../../model/workflowModel';
import {
  WECHAT_MESSAGE_SENT_QUERY,
} from './automation';
import {
  WECHAT_WORKFLOW_EDGES,
  WECHAT_WORKFLOW_INPUTS,
  WECHAT_WORKFLOW_NAME,
  WECHAT_WORKFLOW_NODES,
  WECHAT_WORKFLOW_PERMISSIONS,
  WECHAT_WORKFLOW_VARIABLES,
} from './template';

describe('微信联系人消息示例工作流', () => {
  it('在默认画布展开搜索、选择、发送和结果检查步骤', () => {
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      WECHAT_WORKFLOW_NAME,
      WECHAT_WORKFLOW_INPUTS,
      WECHAT_WORKFLOW_VARIABLES,
      WECHAT_WORKFLOW_PERMISSIONS,
      WECHAT_WORKFLOW_NODES,
      WECHAT_WORKFLOW_EDGES,
    );
    /** 用 ID 定位关键节点，避免测试依赖数组中的视觉排列。 */
    const nodesById = new Map(workflow.nodes.map((node) => [node.id, node]));

    expect(FLOW_COMPONENT_CATALOG).toEqual([]);
    expect(nodesById.get('select_contact')?.payload).toEqual(expect.objectContaining({
      operation: expect.objectContaining({
        type: 'click',
        target: expect.objectContaining({
          locator: expect.objectContaining({
            query: expect.objectContaining({ language_version: 3 }),
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
    expect(workflow.edges).toEqual(expect.arrayContaining([
      expect.objectContaining({
        source: 'check_search',
        target: 'select_search_text',
        branch: 'true',
      }),
      expect.objectContaining({
        source: 'check_conversation',
        target: 'type_message',
        branch: 'true',
      }),
      expect.objectContaining({
        source: 'check_send_result',
        target: 'end',
        branch: 'true',
      }),
      expect.objectContaining({
        source: 'check_send_result',
        target: 'wait_for_send_result',
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
