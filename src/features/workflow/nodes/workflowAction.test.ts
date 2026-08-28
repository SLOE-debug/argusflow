import { describe, expect, it } from 'vitest';

import {
  changeUiOperationKind,
  changeBackendPolicy,
  changeTargetLocatorKind,
  createDefaultUiOperation,
} from './workflowAction';

describe('workflow UI operation transformations', () => {
  it('preserves the target when switching from Click to SetValue', () => {
    const click = createDefaultUiOperation();
    const setValue = changeUiOperationKind(click, 'set_value');

    expect(setValue).toEqual({
      type: 'set_value',
      target: click.target,
      value: { type: 'literal', value: '' },
    });
  });

  it('returns non-semantic locators to automatic backend planning', () => {
    const click = createDefaultUiOperation();
    const forcedUia = changeBackendPolicy(click, 'windows_uia');
    const visual = changeTargetLocatorKind(forcedUia, 'visual');

    expect(visual.target.locator).toEqual({
      type: 'visual',
      query: { text: '确定', exact: true },
    });
    expect(visual.target.backend_policy).toEqual({ allow: [], deny: [], prefer: [] });
  });

  it('forces collect links onto a semantic CDP query', () => {
    const click = createDefaultUiOperation();
    const visual = changeTargetLocatorKind(click, 'visual');

    const collectLinks = changeUiOperationKind(visual, 'collect_links');

    expect(collectLinks.type).toBe('collect_links');
    expect(collectLinks.target.locator.type).toBe('query');
    expect(collectLinks.target.backend_policy).toEqual({
      allow: ['browser_cdp'],
      deny: [],
      prefer: ['browser_cdp'],
    });
  });

  it('forces keyboard actions onto the focused SendInput target', () => {
    const click = createDefaultUiOperation();
    const pressKey = changeUiOperationKind(click, 'press_key');

    expect(pressKey).toEqual({
      type: 'press_key',
      target: {
        ...click.target,
        locator: { type: 'focused' },
        backend_policy: {
          allow: ['send_input'],
          deny: [],
          prefer: ['send_input'],
        },
      },
      chord: { key: { type: 'enter' }, modifiers: [] },
    });
  });
});
