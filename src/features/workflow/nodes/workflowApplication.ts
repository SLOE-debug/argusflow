import type { ApplicationSpec } from '../model/contracts';

/** 默认应用资源使用本机标准 Notepad++ 安装路径。 */
export const DEFAULT_APPLICATION_EXECUTABLE = 'C:\\Program Files\\Notepad++\\notepad++.exe';

/** 创建幂等 AttachOrStart 且默认不清理用户应用的资源契约。 */
export function createDefaultApplicationSpec(): ApplicationSpec {
  return {
    executable_path: DEFAULT_APPLICATION_EXECUTABLE,
    arguments: [],
    window_title: { type: 'contains', value: 'Notepad++' },
    acquire_policy: 'attach_or_start',
    launch_timeout_ms: 10_000,
    cleanup_policy: 'leave_running',
    activation_policy: 'best_effort',
  };
}
