import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

/** 从实际 IPC 注册入口提取命令，检查构建清单和主窗口授权是否同步。 */
const entry = readFileSync(new URL('../src/lib.rs', import.meta.url), 'utf8');
const build = readFileSync(new URL('../build.rs', import.meta.url), 'utf8');
const capability = JSON.parse(readFileSync(new URL('../capabilities/default.json', import.meta.url), 'utf8'));
const commands = [...entry.matchAll(/commands::\w+::(\w+),/g)].map((match) => match[1]);

test('every registered IPC command is declared in the application manifest', () => {
  assert.ok(commands.length > 0, 'IPC command registration must be found');
  for (const command of commands) {
    assert.ok(build.includes(`"${command}"`), `${command} is missing from the build manifest`);
  }
});

test('the main window can invoke all registered application commands', () => {
  assert.deepEqual(capability.windows, ['main']);
  for (const command of commands) {
    const permission = `allow-${command.replaceAll('_', '-')}`;
    assert.ok(capability.permissions.includes(permission), `${permission} is missing from the main capability`);
  }
});
