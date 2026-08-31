import { StrictMode, type PropsWithChildren } from 'react';
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { INITIAL_STARTUP_SNAPSHOT } from './model';
import type { StartupSnapshot } from './model';

type StartupStatusListener = (event: Readonly<{ payload: StartupSnapshot }>) => void;

/** 测试中捕获的 Tauri 状态监听器，用于模拟后台健康轮询。 */
let startupStatusListener: StartupStatusListener | null = null;

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
    startupStatusListener = null;
    startupMocks.beginRuntimeInitialization.mockResolvedValue(INITIAL_STARTUP_SNAPSHOT);
    startupMocks.getStartupStatus.mockResolvedValue(INITIAL_STARTUP_SNAPSHOT);
    startupMocks.retryStartup.mockResolvedValue(INITIAL_STARTUP_SNAPSHOT);
    startupMocks.listen.mockImplementation((
      _eventName: string,
      listener: StartupStatusListener,
    ) => {
      startupStatusListener = listener;
      return Promise.resolve(() => undefined);
    });
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

  it('keeps the existing snapshot when a health poll reports identical content', async () => {
    const { result } = renderHook(() => useStartupStatus());
    await waitFor(() => expect(startupStatusListener).not.toBeNull());
    /** 更新前的对象引用用于确认相同轮询不会向工作台发布新状态。 */
    const currentStatus = result.current.status;

    act(() => {
      startupStatusListener?.({
        payload: {
          ...INITIAL_STARTUP_SNAPSHOT,
          capture: { ...INITIAL_STARTUP_SNAPSHOT.capture },
          smallOcr: { ...INITIAL_STARTUP_SNAPSHOT.smallOcr },
          mediumOcr: { ...INITIAL_STARTUP_SNAPSHOT.mediumOcr },
        },
      });
    });

    expect(result.current.status).toBe(currentStatus);
  });
});
