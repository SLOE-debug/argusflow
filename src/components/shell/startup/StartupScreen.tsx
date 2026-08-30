import ArrowRight from 'lucide-react/dist/esm/icons/arrow-right.mjs';
import CircleCheck from 'lucide-react/dist/esm/icons/circle-check.mjs';
import CircleDashed from 'lucide-react/dist/esm/icons/circle-dashed.mjs';
import CircleX from 'lucide-react/dist/esm/icons/circle-x.mjs';
import Cpu from 'lucide-react/dist/esm/icons/cpu.mjs';
import LoaderCircle from 'lucide-react/dist/esm/icons/loader-circle.mjs';
import MonitorUp from 'lucide-react/dist/esm/icons/monitor-up.mjs';
import RotateCcw from 'lucide-react/dist/esm/icons/rotate-ccw.mjs';
import ScanText from 'lucide-react/dist/esm/icons/scan-text.mjs';
import type { LucideIcon } from 'lucide-react';

import { Button } from '../../ui/button';
import {
  COMPONENT_STATUS_LABELS,
  STARTUP_PHASE_COPY,
  type StartupComponentStatus,
  type StartupSnapshot,
} from '../../../features/startup';

type StartupScreenProps = Readonly<{
  /** 后端当前能力启动快照。 */
  status: StartupSnapshot;
  /** 重试命令是否仍在提交。 */
  retrying?: boolean;
  /** IPC 重试失败时可安全展示的说明。 */
  errorMessage?: string | null;
  /** 触发 WGC 与 OCR 重试。 */
  onRetry?: () => void;
  /** 忽略关键能力失败并进入只编辑工作台。 */
  onContinueDegraded?: () => void;
}>;

/** 首帧即可渲染的轻量启动页，不加载 Monaco、画布或工作流 Store。 */
export function StartupScreen({
  status,
  retrying = false,
  errorMessage = null,
  onRetry,
  onContinueDegraded,
}: StartupScreenProps) {
  const copy = STARTUP_PHASE_COPY[status.phase];
  const blocked = status.readiness === 'blocked';
  const startupFailed = blocked || errorMessage !== null;
  const runtimeMessage = errorMessage
    ?? status.capture.message
    ?? status.smallOcr.message
    ?? status.degradationReason;

  return (
    <main
      data-theme="daylight"
      className="grid h-full w-full grid-rows-[40px_minmax(0,1fr)] bg-slate-50 text-slate-800"
    >
      <header
        data-tauri-drag-region
        className="flex items-center border-b border-slate-200 bg-white px-4 text-xs font-semibold text-slate-700"
      >
        ArgusFlow
      </header>
      <section className="flex min-h-0 items-center justify-center overflow-auto px-6 py-10">
        <div className="w-full max-w-xl rounded-2xl border border-slate-200 bg-white p-8 shadow-sm">
          <div className="mb-7 flex items-start gap-4">
            <div className="relative flex size-11 shrink-0 items-center justify-center rounded-xl bg-blue-50 text-blue-600">
              <span
                className="absolute inset-1 animate-ping rounded-lg bg-blue-200/70 motion-reduce:animate-none"
                aria-hidden="true"
              />
              <LoaderCircle
                className="relative size-5 animate-spin motion-reduce:animate-none"
                aria-hidden="true"
              />
            </div>
            <div className="min-w-0">
              <h1 className="text-lg font-semibold tracking-tight text-slate-900">
                {copy.title}
              </h1>
              <p className="mt-1 text-sm leading-6 text-slate-500">{copy.detail}</p>
            </div>
          </div>

          <div
            className="mb-6 grid grid-cols-3 gap-2"
            role="progressbar"
            aria-label="本地运行环境启动进度"
            aria-valuemin={0}
            aria-valuemax={status.totalSteps}
            aria-valuenow={status.completedSteps}
          >
            {[status.capture, status.smallOcr, status.mediumOcr].map((component, index) => (
              <span
                key={index}
                className={`h-1.5 rounded-full ${progressTone(component)}`}
              />
            ))}
          </div>

          <div className="divide-y divide-slate-100 rounded-xl border border-slate-200">
            <StartupCapability
              icon={MonitorUp}
              label="屏幕捕获"
              status={status.capture}
            />
            <StartupCapability
              icon={ScanText}
              label="快速 OCR"
              detail={deviceLabel(status)}
              status={status.smallOcr}
            />
            <StartupCapability
              icon={Cpu}
              label="精确 OCR"
              detail="完成后进入 Home"
              status={status.mediumOcr}
            />
          </div>

          {runtimeMessage ? (
            <p
              className={`mt-5 rounded-lg px-3 py-2.5 text-xs leading-5 ${
                blocked
                  ? 'bg-rose-50 text-rose-700'
                  : 'bg-amber-50 text-amber-700'
              }`}
              role={startupFailed ? 'alert' : 'status'}
            >
              {runtimeMessage}
            </p>
          ) : null}

          {startupFailed ? (
            <div className="mt-6">
              <p className="mb-3 text-xs leading-5 text-slate-500">
                降级模式仍可编辑和检查流程，运行操作暂不可用。
              </p>
              <div className="flex items-center gap-2">
                <Button
                  variant="primary"
                  icon={RotateCcw}
                  loading={retrying}
                  loadingLabel="正在重试…"
                  onClick={onRetry}
                >
                  重试启动
                </Button>
                <Button
                  variant="secondary"
                  icon={ArrowRight}
                  onClick={onContinueDegraded}
                >
                  进入降级模式
                </Button>
              </div>
            </div>
          ) : null}
        </div>
      </section>
    </main>
  );
}

type StartupCapabilityProps = Readonly<{
  /** 能力类别图标。 */
  icon: LucideIcon;
  /** 能力名称。 */
  label: string;
  /** 设备或非阻塞说明。 */
  detail?: string;
  /** 当前生命周期。 */
  status: StartupComponentStatus;
}>;

/** 启动卡片中的单项能力状态。 */
function StartupCapability({
  icon: Icon,
  label,
  detail,
  status,
}: StartupCapabilityProps) {
  const StatusIcon = status.lifecycle === 'ready'
    ? CircleCheck
    : status.lifecycle === 'failed'
      ? CircleX
      : CircleDashed;
  const iconTone = status.lifecycle === 'ready'
    ? 'text-emerald-600'
    : status.lifecycle === 'failed'
      ? 'text-rose-600'
    : 'animate-spin text-blue-500 motion-reduce:animate-none';

  return (
    <div className="flex min-h-14 items-center gap-3 px-4 py-3">
      <Icon
        className="size-4 shrink-0 text-slate-400"
        aria-hidden="true"
      />
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-slate-700">{label}</p>
        {detail ? <p className="mt-0.5 text-xs text-slate-400">{detail}</p> : null}
      </div>
      <StatusIcon
        className={`size-4 shrink-0 ${iconTone}`}
        aria-hidden="true"
      />
      <span className="w-14 text-right text-xs text-slate-500">
        {COMPONENT_STATUS_LABELS[status.lifecycle]}
      </span>
    </div>
  );
}

/** 用固定 Tailwind 色段表达三项离散启动进度。 */
function progressTone(status: StartupComponentStatus): string {
  if (status.lifecycle === 'ready') return 'bg-emerald-500';
  if (status.lifecycle === 'failed') return 'bg-rose-400';
  if (status.lifecycle === 'pending') {
    return 'animate-pulse bg-blue-200 motion-reduce:animate-none';
  }
  return 'animate-pulse bg-blue-500 motion-reduce:animate-none';
}

/** 显示已实测通过的推理设备。 */
function deviceLabel(status: StartupSnapshot): string | undefined {
  if (!status.device) return undefined;
  return status.device.kind === 'cuda'
    ? `GPU ${status.device.index} 加速`
    : 'CPU 推理';
}
