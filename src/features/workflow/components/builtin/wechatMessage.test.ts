import { describe, expect, it } from 'vitest';

import { createWechatMessageDefinition } from './wechatMessage';

describe('WeChat message component', () => {
  it('binds opening and sending confirmation to the right-side conversation', () => {
    const definition = createWechatMessageDefinition();
    const nodeIds = definition.nodes.map((node) => node.id);
    const clickContact = definition.nodes.find((node) => node.id === 'click_contact');
    const sendMessage = definition.nodes.find((node) => node.id === 'send_message');

    expect(definition.version).toBe('5.0.1');
    expect(
      definition.nodes
        .filter((node) => node.type_id === 'argus.ui')
        .every((node) => node.version === 4),
    ).toBe(true);
    expect(nodeIds).not.toContain('find_contact');
    expect(nodeIds).not.toContain('verify_header');
    expect(definition.edges).toContainEqual(expect.objectContaining({
      source: 'type_contact',
      target: 'click_contact',
    }));
    expect(definition.edges).toContainEqual(expect.objectContaining({
      source: 'click_contact',
      target: 'type_message',
    }));
    expect(clickContact?.payload).toEqual(expect.objectContaining({
      execution: expect.objectContaining({
        postcondition: {
          type: 'match_present',
          query: expect.objectContaining({
            source: 'nearest(anchor = text(name = "搜索"), target = text(name = $contact_name), direction = any, index = 2)',
          }),
        },
      }),
    }));
    expect(sendMessage?.payload).toEqual(expect.objectContaining({
      execution: expect.objectContaining({
        postcondition: expect.objectContaining({
          type: 'match_removed',
          query: expect.objectContaining({
            source: 'nearest(anchor = viewport_edge(side = bottom), target = text(name = $message), direction = any, index = 1)',
          }),
          stable_context: [expect.objectContaining({
            source: 'nearest(anchor = text(name = "搜索"), target = text(name = $contact_name), direction = any, index = 2)',
          })],
        }),
      }),
    }));
  });
});
