import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';

import type {
  CommandOperation,
  CommandRunner,
} from '../../features/workflow/contracts';
import { CommandNodeFields } from './CommandNodeFields';

/** 创建字段完整的 shell 命令，供 runner 差异测试复用。 */
function createShellOperation(runner: Exclude<CommandRunner, 'direct'>): CommandOperation {
  return {
    runner,
    program: null,
    arguments: [],
    script: { type: 'literal', value: 'echo first' },
    working_directory: null,
    environment: [],
    stdin: null,
    timeout_ms: 30_000,
    accepted_exit_codes: [0],
    max_stdout_bytes: 1_048_576,
    max_stderr_bytes: 1_048_576,
  };
}

describe('CommandNodeFields', () => {
  it.each([
    ['power_shell', 'PowerShell'],
    ['cmd', 'CMD'],
  ] as const)('edits %s literal scripts as multiline text', (runner, badge) => {
    const onChange = vi.fn();
    const operation = createShellOperation(runner);
    render(<CommandNodeFields nodeId="shell-command" operation={operation} onChange={onChange} />);

    const script = screen.getByRole('textbox', { name: '脚本内容' });
    expect(script).toHaveAttribute(
      'data-language',
      runner === 'power_shell' ? 'powershell' : 'bat',
    );
    const scriptHeading = screen.getByRole('heading', { name: '脚本' });
    expect(scriptHeading.nextElementSibling).toHaveTextContent(badge);

    fireEvent.change(script, { target: { value: 'echo first\necho second' } });

    expect(onChange).toHaveBeenCalledWith({
      ...operation,
      script: { type: 'literal', value: 'echo first\necho second' },
    });
  });

  it('keeps non-literal script sources as reference fields', () => {
    const operation: CommandOperation = {
      ...createShellOperation('power_shell'),
      script: { type: 'workflow_input', key: 'maintenance_script' },
    };
    render(<CommandNodeFields nodeId="shell-reference" operation={operation} onChange={vi.fn()} />);

    expect(screen.queryByRole('textbox', { name: '脚本内容' })).not.toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: '工作流输入字段' })).toHaveValue(
      'maintenance_script',
    );
    expect(screen.queryByRole('button', { name: '展开编辑脚本' })).not.toBeInTheDocument();
  });

  it('keeps Direct program editing single-line', () => {
    const operation: CommandOperation = {
      ...createShellOperation('cmd'),
      runner: 'direct',
      program: { type: 'literal', value: 'whoami.exe' },
      script: null,
    };
    render(<CommandNodeFields nodeId="direct-command" operation={operation} onChange={vi.fn()} />);

    const program = screen.getByRole('textbox', { name: '程序' });
    expect(program.tagName).toBe('INPUT');
    expect(screen.queryByRole('textbox', { name: '脚本内容' })).not.toBeInTheDocument();
  });

  it('preserves the script value when switching shell runners', () => {
    function RunnerHarness() {
      const [operation, setOperation] = useState(createShellOperation('power_shell'));
      return (
        <CommandNodeFields
          nodeId="runner-switch"
          operation={operation}
          onChange={setOperation}
        />
      );
    }

    render(<RunnerHarness />);
    fireEvent.change(screen.getAllByRole('combobox')[0], { target: { value: 'cmd' } });

    expect(screen.getByRole('textbox', { name: '脚本内容' })).toHaveValue('echo first');
    const scriptHeading = screen.getByRole('heading', { name: '脚本' });
    expect(scriptHeading.nextElementSibling).toHaveTextContent('CMD');
  });
});
