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

  it('routes AQL clicks through OCR scene materialization and SendInput', () => {
    const click = createDefaultUiOperation();
    const visual = changeBackendPolicy(click, 'ocr_small');

    expect(visual.target.locator.type).toBe('query');
    expect(visual.target.backend_policy).toEqual({
      allow: ['ocr_small', 'send_input'],
      deny: [],
      prefer: ['ocr_small', 'send_input'],
    });
  });

  it('keeps coordinate clicks on their explicit coordinate target', () => {
    const click = createDefaultUiOperation();
    const coordinateClick = changeTargetLocatorKind(click, 'coordinate');

    expect(coordinateClick.type).toBe('click');
    expect(coordinateClick.target.locator.type).toBe('coordinate');
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
