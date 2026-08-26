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
        homeActive={false}
        onOpenHome={vi.fn()}
        onOpenWorkflow={vi.fn()}
      />,
    );

    fireEvent.mouseDown(screen.getByText('ArgusFlow Studio').parentElement!, {
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
        homeActive={false}
        onOpenHome={vi.fn()}
        onOpenWorkflow={vi.fn()}
      />,
    );

    fireEvent.mouseDown(screen.getByText('ArgusFlow Studio').parentElement!, {
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
        homeActive={false}
        onOpenHome={vi.fn()}
        onOpenWorkflow={vi.fn()}
      />,
    );

    expect(await screen.findByRole('button', { name: '还原窗口' })).toBeEnabled();
  });

  it('centers the saved status with flex layout', () => {
    render(
      <WindowTitleBar
        workflowName="测试流程"
        running={false}
        report={null}
        errorMessage={null}
        homeActive={false}
        onOpenHome={vi.fn()}
        onOpenWorkflow={vi.fn()}
      />,
    );

    const statusText = screen.getByText(/已保存/);
    expect(statusText.parentElement).toHaveClass('flex', 'h-[26px]', 'items-center');
    expect(statusText.previousElementSibling).not.toHaveClass('translate-y-px');
  });

  it('opens home and the current workflow from title-bar navigation', () => {
    const onOpenHome = vi.fn();
    const onOpenWorkflow = vi.fn();
    render(
      <WindowTitleBar
        workflowName="测试流程"
        running={false}
        report={null}
        errorMessage={null}
        homeActive
        onOpenHome={onOpenHome}
        onOpenWorkflow={onOpenWorkflow}
      />,
    );

    const homeButton = screen.getByRole('button', { name: '打开工作区概览' });
    expect(homeButton).toHaveAttribute('aria-current', 'page');
    fireEvent.click(homeButton);
    fireEvent.click(screen.getByRole('button', { name: '打开工作流 测试流程' }));

    expect(onOpenHome).toHaveBeenCalledOnce();
    expect(onOpenWorkflow).toHaveBeenCalledOnce();
  });

  it('hosts editor commands without turning button clicks into window drags', () => {
    const onRun = vi.fn();
    render(
      <WindowTitleBar
        workflowName="测试流程"
        running={false}
        report={null}
        errorMessage={null}
        homeActive={false}
        editorCommands={<button type="button">撤销</button>}
        editorActions={<button type="button" onClick={onRun}>运行</button>}
        onOpenHome={vi.fn()}
        onOpenWorkflow={vi.fn()}
      />,
    );

    fireEvent.mouseDown(screen.getByRole('button', { name: '运行' }), {
      button: 0,
      detail: 1,
    });
    fireEvent.click(screen.getByRole('button', { name: '运行' }));

    expect(screen.getByRole('button', { name: '撤销' })).toBeVisible();
    expect(onRun).toHaveBeenCalledOnce();
    expect(windowMock.startDragging).not.toHaveBeenCalled();
  });
});
