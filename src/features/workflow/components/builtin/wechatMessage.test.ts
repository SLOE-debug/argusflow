import { describe, expect, it } from 'vitest';

import { createWechatMessageDefinition } from './wechatMessage';

describe('WeChat message component', () => {
  it('binds opening and sending confirmation to the right-side conversation', () => {
    const definition = createWechatMessageDefinition();
    const nodeIds = definition.nodes.map((node) => node.id);
    const clickContact = definition.nodes.find((node) => node.id === 'click_contact');
    const sendMessage = definition.nodes.find((node) => node.id === 'send_message');

    expect(definition.version).toBe('4.0.0');
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
        postcondition: expect.objectContaining({ type: 'text_present' }),
      }),
    }));
    expect(sendMessage?.payload).toEqual(expect.objectContaining({
      execution: expect.objectContaining({
        postcondition: expect.objectContaining({
          type: 'new_text',
          stable_context: [expect.objectContaining({ exact: true })],
        }),
      }),
    }));
  });
});
