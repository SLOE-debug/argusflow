import { describe, expect, it } from 'vitest';

import {
  changeUiOperationKind,
  changeBackendPreference,
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
    const forcedUia = changeBackendPreference(click, 'windows_uia');
    const visual = changeTargetLocatorKind(forcedUia, 'visual');

    expect(visual.target.locator).toEqual({
      type: 'visual',
      query: { text: '确定', exact: true },
    });
    expect(visual.target.backend_preference).toBe('auto');
  });
});
