import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { Input, Select } from './index';

describe('form controls', () => {
  it('shares compact text sizing between Input and Select', () => {
    render(
      <>
        <Input
          aria-label="搜索"
          density="compact"
        />
        <Select
          aria-label="工作区"
          density="compact"
          value="default"
          options={[{ value: 'default', label: '默认工作区' }]}
        />
      </>,
    );

    expect(screen.getByRole('textbox', { name: '搜索' })).toHaveClass('text-[12px]');
    expect(screen.getByRole('combobox', { name: '工作区' })).toHaveClass('text-[12px]');
  });

  it('returns only values declared by Select options', () => {
    const onValueChange = vi.fn();

    render(
      <Select
        aria-label="工作区"
        value="default"
        options={[
          { value: 'default', label: '默认工作区' },
          { value: 'team', label: '团队工作区' },
        ]}
        onValueChange={onValueChange}
      />,
    );

    fireEvent.change(screen.getByRole('combobox', { name: '工作区' }), {
      target: { value: 'team' },
    });

    expect(onValueChange).toHaveBeenCalledWith('team');
  });
});
