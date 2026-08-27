import type { CommandOperation, CommandRunner } from '../model/contracts';

/** 创建默认无 shell 的命令执行契约。 */
export function createDefaultCommandOperation(): CommandOperation {
  return {
    runner: 'direct',
    program: { type: 'literal', value: 'C:\\Windows\\System32\\whoami.exe' },
    arguments: [],
    script: null,
    working_directory: null,
    environment: [],
    stdin: null,
    timeout_ms: 30_000,
    accepted_exit_codes: [0],
    max_stdout_bytes: 1_048_576,
    max_stderr_bytes: 1_048_576,
  };
}

/** 切换命令运行器并建立符合判别契约的字段组合。 */
export function changeCommandRunner(
  operation: CommandOperation,
  runner: CommandRunner,
): CommandOperation {
  if (runner === 'direct') {
    return {
      ...operation,
      runner,
      program: operation.program ?? { type: 'literal', value: '' },
      script: null,
    };
  }
  return {
    ...operation,
    runner,
    program: null,
    arguments: [],
    script: operation.script ?? { type: 'literal', value: '' },
  };
}
