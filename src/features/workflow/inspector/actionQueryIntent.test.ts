import { describe, expect, it } from 'vitest';

import type { AqlQuery } from '../model/contracts';
import {
  changeQueryControlRole,
  changeQueryTargetMatch,
  changeQueryTargetText,
  readQueryTargetIntent,
} from './actionQueryIntent';

describe('actionQueryIntent', () => {
  it('maps and edits a simple text target without exposing engine vocabulary', () => {
    const query: AqlQuery = {
      language_version: 3,
      source: 'text(name contains "选择联系人")',
      bindings: {},
    };

    expect(readQueryTargetIntent(query)).toEqual({
      type: 'text',
      value: { source: 'literal', text: '选择联系人' },
      match: 'contains',
      editable: true,
      hasMoreConditions: false,
    });
    expect(changeQueryTargetText(query, '联系人').source).toBe(
      'text(name contains "联系人")',
    );
    expect(changeQueryTargetMatch(query, 'exact').source).toBe(
      'text(name = "选择联系人")',
    );
  });

  it('finds a bound target inside nearest while preserving its spatial conditions', () => {
    const query: AqlQuery = {
      language_version: 3,
      source: 'nearest(anchor = text(name = "搜索"), target = text(name = $contact), direction = below, index = 1)',
      bindings: {
        contact: {
          type: 'ref',
          source: { type: 'workflow_input', key: '联系人' },
          pointer: '',
        },
      },
    };

    expect(readQueryTargetIntent(query)).toEqual({
      type: 'text',
      value: {
        source: 'binding',
        bindingName: 'contact',
        text: '流程输入「联系人」',
      },
      match: 'exact',
      editable: false,
      hasMoreConditions: false,
    });
    expect(changeQueryTargetText(query, '不会覆盖绑定')).toBe(query);
  });

  it('preserves extra control conditions when editing the visible role and name', () => {
    const query: AqlQuery = {
      language_version: 3,
      source: 'button(name = "保存", enabled = true)',
      bindings: {},
    };

    expect(readQueryTargetIntent(query)).toMatchObject({
      type: 'control',
      role: 'button',
      hasMoreConditions: true,
    });
    expect(changeQueryControlRole(query, 'menu_item').source).toBe(
      'menu_item(name = "保存", enabled = true)',
    );
    expect(changeQueryTargetText(query, '另存为').source).toBe(
      'button(name = "另存为", enabled = true)',
    );
  });
});
