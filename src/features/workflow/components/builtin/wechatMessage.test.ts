import { describe, expect, it } from 'vitest';

import { createWechatMessageDefinition } from './wechatMessage';

describe('WeChat message component', () => {
  it('queries and clicks the transient contact result in one action', () => {
    const definition = createWechatMessageDefinition();
    const nodeIds = definition.nodes.map((node) => node.id);

    expect(definition.version).toBe('3.1.0');
    expect(nodeIds).not.toContain('find_contact');
    expect(definition.edges).toContainEqual(expect.objectContaining({
      source: 'type_contact',
      target: 'click_contact',
    }));
  });
});
