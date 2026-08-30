import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import {
  INITIAL_STARTUP_SNAPSHOT,
  type StartupSnapshot,
} from '../../../features/startup';
import { StartupScreen } from './StartupScreen';

/** Small OCR 失败时的确定阻塞快照。 */
const blockedStatus: StartupSnapshot = {
  readiness: 'blocked',
  phase: 'failed',
  completedSteps: 1,
  totalSteps: 3,
  capture: { lifecycle: 'ready', message: null },
  smallOcr: { lifecycle: 'failed', message: 'Small OCR model failed' },
  mediumOcr: { lifecycle: 'pending', message: null },
  device: { kind: 'cpu' },
  degradationReason: null,
};

describe('StartupScreen', () => {
  it('renders active loading motion while desktop capabilities are pending', () => {
    const { container } = render(
      <StartupScreen status={INITIAL_STARTUP_SNAPSHOT} />,
    );

    const progress = screen.getByRole('progressbar');
    expect(progress).toHaveAttribute('aria-valuenow', '0');
    expect(screen.getByText('三项能力并行准备中')).toBeInTheDocument();
    expect(container.querySelector('.animate-ping')).toBeInTheDocument();
    expect(container.querySelector('.animate-spin')).toBeInTheDocument();
    expect(progress.querySelectorAll('.animate-pulse')).toHaveLength(3);
  });

  it('explains blocked startup and exposes both recovery choices', () => {
    const onRetry = vi.fn();
    const onContinueDegraded = vi.fn();
    render(
      <StartupScreen
        status={blockedStatus}
        onRetry={onRetry}
        onContinueDegraded={onContinueDegraded}
      />,
    );

    expect(screen.getByRole('alert')).toHaveTextContent('Small OCR model failed');
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '1');
    fireEvent.click(screen.getByRole('button', { name: '重试启动' }));
    fireEvent.click(screen.getByRole('button', { name: '进入降级模式' }));
    expect(onRetry).toHaveBeenCalledOnce();
    expect(onContinueDegraded).toHaveBeenCalledOnce();
  });
});
