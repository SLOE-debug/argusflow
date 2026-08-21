import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { WindowTitleBar } from './WindowTitleBar';

/** Tauri 当前窗口 API 的可观测测试替身。 */
const windowMock = vi.hoisted(() => ({
  close: vi.fn(),
  isMaximized: vi.fn(),
  minimize: vi.fn(),
  onResized: vi.fn(),
  startDragging: vi.fn(),
  toggleMaximize: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => windowMock,
}));

describe('WindowTitleBar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    windowMock.close.mockResolvedValue(undefined);
    windowMock.isMaximized.mockResolvedValue(false);
    windowMock.minimize.mockResolvedValue(undefined);
    windowMock.onResized.mockResolvedValue(vi.fn());
    windowMock.startDragging.mockResolvedValue(undefined);
    windowMock.toggleMaximize.mockResolvedValue(undefined);
  });

  it('drags the title area and exposes native window controls', async () => {
    render(
      <WindowTitleBar
        workflowName="测试流程"
        running={false}
        report={null}
        errorMessage={null}
      />,
    );

    fireEvent.mouseDown(screen.getByText('测试流程').parentElement!, {
      button: 0,
      detail: 1,
    });
    fireEvent.click(screen.getByRole('button', { name: '最小化窗口' }));
    fireEvent.click(screen.getByRole('button', { name: '最大化窗口' }));
    fireEvent.click(screen.getByRole('button', { name: '关闭窗口' }));

    expect(windowMock.startDragging).toHaveBeenCalledOnce();
    expect(windowMock.minimize).toHaveBeenCalledOnce();
    expect(windowMock.toggleMaximize).toHaveBeenCalledOnce();
    expect(windowMock.close).toHaveBeenCalledOnce();
    await waitFor(() => expect(windowMock.isMaximized).toHaveBeenCalled());
  });

  it('toggles maximize on title-area double click', () => {
    render(
      <WindowTitleBar
        workflowName="测试流程"
        running={false}
        report={null}
        errorMessage={null}
      />,
    );

    fireEvent.mouseDown(screen.getByText('测试流程').parentElement!, {
      button: 0,
      detail: 2,
    });

    expect(windowMock.toggleMaximize).toHaveBeenCalledOnce();
    expect(windowMock.startDragging).not.toHaveBeenCalled();
  });

  it('shows the restore command for a maximized window', async () => {
    windowMock.isMaximized.mockResolvedValue(true);
    render(
      <WindowTitleBar
        workflowName="测试流程"
        running={false}
        report={null}
        errorMessage={null}
      />,
    );

    expect(await screen.findByRole('button', { name: '还原窗口' })).toBeEnabled();
  });
});
