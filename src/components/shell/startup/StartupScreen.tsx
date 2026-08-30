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
import { StartupActivity } from './StartupActivity';

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
      className="relative isolate grid h-full w-full grid-rows-[40px_minmax(0,1fr)] overflow-hidden bg-[radial-gradient(circle_at_50%_28%,#eff6ff_0%,#f8fafc_42%,#eef2f7_100%)] text-slate-800"
    >
      <div
        className="pointer-events-none absolute -left-24 top-24 size-80 rounded-full bg-blue-200/25 blur-3xl animate-pulse [animation-duration:5s] motion-reduce:animate-none"
        aria-hidden="true"
      />
      <div
        className="pointer-events-none absolute -right-20 bottom-[-8rem] size-96 rounded-full bg-cyan-200/20 blur-3xl animate-pulse [animation-delay:-2s] [animation-duration:6s] motion-reduce:animate-none"
        aria-hidden="true"
      />
      <header
        data-tauri-drag-region
        className="relative z-10 flex items-center border-b border-white/80 bg-white/75 px-4 text-xs font-semibold text-slate-700 shadow-[0_1px_0_rgba(148,163,184,0.14)] backdrop-blur-xl"
      >
        <span
          className="relative mr-2 flex size-2 items-center justify-center"
          aria-hidden="true"
        >
          <span className="absolute size-2 rounded-full bg-blue-500 animate-ping motion-reduce:animate-none" />
          <span className="relative size-1.5 rounded-full bg-blue-600" />
        </span>
        ArgusFlow Studio
        <span className="mx-2 h-3 w-px bg-slate-200" aria-hidden="true" />
        <span className="font-normal text-slate-400">本地运行环境</span>
      </header>
      <section className="relative z-10 flex min-h-0 items-center justify-center overflow-auto px-6 py-10">
        <div className="w-full max-w-2xl rounded-[28px] border border-white/90 bg-white/75 p-7 shadow-[0_28px_80px_rgba(51,65,85,0.14),0_4px_18px_rgba(37,99,235,0.06)] backdrop-blur-xl sm:p-9">
          <div className="grid items-center gap-7 sm:grid-cols-[176px_minmax(0,1fr)]">
            <StartupActivity
              capture={status.capture}
              smallOcr={status.smallOcr}
              mediumOcr={status.mediumOcr}
            />
            <div
              className="min-w-0 text-center sm:text-left"
              role="status"
              aria-live="polite"
            >
              <div className="mb-3 inline-flex items-center gap-2 rounded-full border border-blue-100 bg-blue-50/80 px-3 py-1 text-[11px] font-medium text-blue-700">
                {status.readiness === 'loading' ? (
                  <LoaderCircle
                    className="size-3.5 animate-spin motion-reduce:animate-none"
                    aria-hidden="true"
                  />
                ) : startupFailed ? (
                  <CircleX
                    className="size-3.5 text-rose-500"
                    aria-hidden="true"
                  />
                ) : (
                  <CircleCheck
                    className="size-3.5 text-emerald-500"
                    aria-hidden="true"
                  />
                )}
                {status.readiness === 'loading'
                  ? '三项能力并行准备中'
                  : startupFailed
                    ? '启动需要处理'
                    : '本地能力已就绪'}
              </div>
              <h1 className="text-xl font-semibold tracking-[-0.025em] text-slate-950 sm:text-2xl">
                {copy.title}
              </h1>
              <p className="mt-2 text-sm leading-6 text-slate-500">{copy.detail}</p>
            </div>
          </div>

          <div
            className="my-7 grid grid-cols-3 gap-2"
            role="progressbar"
            aria-label="本地运行环境启动进度"
            aria-valuemin={0}
            aria-valuemax={status.totalSteps}
            aria-valuenow={status.completedSteps}
          >
            {[status.capture, status.smallOcr, status.mediumOcr].map((component, index) => (
              <span
                key={index}
                className={`h-1.5 overflow-hidden rounded-full shadow-inner ${progressTone(component)}`}
              />
            ))}
          </div>

          <div className="divide-y divide-slate-100/90 overflow-hidden rounded-2xl border border-slate-200/80 bg-white/75 shadow-[0_1px_0_rgba(255,255,255,0.8)]">
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
              detail="复杂页面识别"
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
    <div className="flex min-h-[62px] items-center gap-3 px-4 py-3 transition-colors">
      <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-slate-50 text-slate-400 ring-1 ring-slate-100">
        <Icon
          className="size-4"
          aria-hidden="true"
        />
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-slate-700">{label}</p>
        {detail ? <p className="mt-0.5 text-xs text-slate-400">{detail}</p> : null}
      </div>
      <span
        className={`flex size-7 shrink-0 items-center justify-center rounded-full bg-white shadow-sm ring-1 ring-slate-100 ${iconTone}`}
      >
        <StatusIcon
          className="size-4"
          aria-hidden="true"
        />
      </span>
      <span className="w-14 text-right text-xs font-medium text-slate-500">
        {COMPONENT_STATUS_LABELS[status.lifecycle]}
      </span>
    </div>
  );
}

/** 用固定 Tailwind 色段表达三项离散启动进度。 */
function progressTone(status: StartupComponentStatus): string {
  if (status.lifecycle === 'ready') return 'bg-emerald-500 shadow-emerald-200';
  if (status.lifecycle === 'failed') return 'bg-rose-400 shadow-rose-200';
  if (status.lifecycle === 'pending') {
    return 'animate-pulse bg-slate-200 motion-reduce:animate-none';
  }
  return 'animate-pulse bg-gradient-to-r from-blue-400 via-cyan-400 to-blue-500 shadow-blue-200 motion-reduce:animate-none';
}

/** 显示已实测通过的推理设备。 */
function deviceLabel(status: StartupSnapshot): string | undefined {
  if (!status.device) return undefined;
  return status.device.kind === 'cuda'
    ? `GPU ${status.device.index} 加速`
    : 'CPU 推理';
}
