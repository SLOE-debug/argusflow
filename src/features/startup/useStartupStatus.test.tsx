import { StrictMode, type PropsWithChildren } from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { INITIAL_STARTUP_SNAPSHOT } from './model';

/** Tauri 边界全部替换为可观察的前端编排测试桩。 */
const startupMocks = vi.hoisted(() => ({
  beginRuntimeInitialization: vi.fn(),
  getStartupStatus: vi.fn(),
  retryStartup: vi.fn(),
  listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: startupMocks.listen,
}));

vi.mock('./startupApi', () => ({
  beginRuntimeInitialization: startupMocks.beginRuntimeInitialization,
  getStartupStatus: startupMocks.getStartupStatus,
  hasDesktopRuntime: () => true,
  retryStartup: startupMocks.retryStartup,
}));

import { useStartupStatus } from './useStartupStatus';

describe('useStartupStatus', () => {
  beforeEach(() => {
    startupMocks.beginRuntimeInitialization.mockReset();
    startupMocks.getStartupStatus.mockReset();
    startupMocks.retryStartup.mockReset();
    startupMocks.listen.mockReset();
    startupMocks.beginRuntimeInitialization.mockResolvedValue(INITIAL_STARTUP_SNAPSHOT);
    startupMocks.getStartupStatus.mockResolvedValue(INITIAL_STARTUP_SNAPSHOT);
    startupMocks.retryStartup.mockResolvedValue(INITIAL_STARTUP_SNAPSHOT);
    startupMocks.listen.mockResolvedValue(() => undefined);
  });

  /** 模拟应用实际使用的 StrictMode，验证探测 effect 不会重复初始化。 */
  const StrictModeWrapper = ({ children }: PropsWithChildren) => (
    <StrictMode>{children}</StrictMode>
  );

  it('starts desktop capabilities immediately after the React loading screen commits', async () => {
    renderHook(() => useStartupStatus(), { wrapper: StrictModeWrapper });

    await waitFor(() => expect(startupMocks.beginRuntimeInitialization).toHaveBeenCalledOnce());
    expect(startupMocks.getStartupStatus).toHaveBeenCalled();
  });
});
