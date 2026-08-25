import { describe, expect, it } from 'vitest';

import {
  changeAutomationActionKind,
  changeBackendPreference,
  changeTargetLocatorKind,
  createDefaultAutomationAction,
} from './workflowAction';

describe('workflow Action transformations', () => {
  it('preserves the target when switching from Click to SetValue', () => {
    const click = createDefaultAutomationAction();
    const setValue = changeAutomationActionKind(click, 'set_value');

    expect(setValue).toEqual({
      type: 'set_value',
      target: click.target,
      value: '',
    });
  });

  it('returns non-semantic locators to automatic backend planning', () => {
    const click = createDefaultAutomationAction();
    const forcedUia = changeBackendPreference(click, 'windows_uia');
    const visual = changeTargetLocatorKind(forcedUia, 'visual');

    expect(visual.target.locator).toEqual({
      type: 'visual',
      query: { text: '确定', exact: true },
    });
    expect(visual.target.backend_preference).toBe('auto');
  });
});
