import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { Checkbox, Input, Select, Textarea } from './index';

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

  it('provides the shared focus and disabled treatment for multiline input', () => {
    render(<Textarea aria-label="查询源码" disabled />);

    expect(screen.getByRole('textbox', { name: '查询源码' })).toHaveClass(
      'focus:border-blue-400',
      'disabled:cursor-not-allowed',
    );
  });

  it('keeps checkbox semantics while applying the shared compact treatment', () => {
    render(<Checkbox aria-label="允许启动进程" defaultChecked />);

    expect(screen.getByRole('checkbox', { name: '允许启动进程' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: '允许启动进程' })).toHaveClass(
      'size-3.5',
      'accent-blue-600',
    );
  });
});
