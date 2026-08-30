import Cpu from 'lucide-react/dist/esm/icons/cpu.mjs';
import MonitorUp from 'lucide-react/dist/esm/icons/monitor-up.mjs';
import ScanText from 'lucide-react/dist/esm/icons/scan-text.mjs';
import Workflow from 'lucide-react/dist/esm/icons/workflow.mjs';
import type { LucideIcon } from 'lucide-react';

import type { StartupComponentStatus } from '../../../features/startup';

type StartupActivityProps = Readonly<{
  /** 屏幕捕获初始化状态。 */
  capture: StartupComponentStatus;
  /** 快速 OCR 模型初始化状态。 */
  smallOcr: StartupComponentStatus;
  /** 精确 OCR 模型初始化状态。 */
  mediumOcr: StartupComponentStatus;
}>;

/** 用三个独立活动节点表达并行启动，不暗示串行百分比。 */
export function StartupActivity({
  capture,
  smallOcr,
  mediumOcr,
}: StartupActivityProps) {
  return (
    <div
      className="relative mx-auto flex size-40 shrink-0 items-center justify-center"
      aria-hidden="true"
    >
      <div className="absolute inset-4 rounded-full bg-blue-300/20 blur-2xl animate-pulse motion-reduce:animate-none" />
      <div className="absolute inset-1 rounded-full border border-dashed border-blue-300/70 animate-spin [animation-duration:14s] motion-reduce:animate-none" />
      <div className="absolute inset-5 rounded-full border border-blue-200/80 animate-spin [animation-direction:reverse] [animation-duration:9s] motion-reduce:animate-none">
        <span className="absolute -top-1 left-1/2 size-2 -translate-x-1/2 rounded-full bg-blue-500 shadow-[0_0_12px_rgba(59,130,246,0.8)]" />
      </div>

      <div className="relative flex size-[72px] items-center justify-center rounded-[22px] border border-white/90 bg-white/90 text-blue-600 shadow-[0_18px_48px_rgba(37,99,235,0.2)] backdrop-blur">
        <span className="absolute inset-2 rounded-2xl bg-blue-100/70 animate-ping [animation-duration:2.4s] motion-reduce:animate-none" />
        <Workflow className="relative size-7" />
      </div>

      <ActivityNode
        className="left-0 top-[26px]"
        icon={MonitorUp}
        status={capture}
      />
      <ActivityNode
        className="right-0 top-[26px]"
        icon={ScanText}
        status={smallOcr}
      />
      <ActivityNode
        className="bottom-0 left-1/2 -translate-x-1/2"
        icon={Cpu}
        status={mediumOcr}
      />
    </div>
  );
}

type ActivityNodeProps = Readonly<{
  /** 节点相对中心轨道的位置。 */
  className: string;
  /** 节点所代表的能力图标。 */
  icon: LucideIcon;
  /** 能力的实时启动状态。 */
  status: StartupComponentStatus;
}>;

/** 轨道上的单项能力节点；颜色与呼吸动效共同传达当前状态。 */
function ActivityNode({ className, icon: Icon, status }: ActivityNodeProps) {
  const active = status.lifecycle === 'initializing' || status.lifecycle === 'warming';
  const tone = activityNodeTone(status);

  return (
    <span
      className={`absolute flex size-10 items-center justify-center rounded-xl border shadow-lg backdrop-blur ${tone} ${className}`}
    >
      {active ? (
        <span className="absolute inset-1 rounded-lg bg-blue-300/50 animate-ping [animation-duration:1.8s] motion-reduce:animate-none" />
      ) : null}
      <Icon className={`relative size-4 ${active ? 'animate-pulse motion-reduce:animate-none' : ''}`} />
    </span>
  );
}

/** 为轨道节点返回完整的静态 Tailwind 状态色，确保构建器能够收集类名。 */
function activityNodeTone(status: StartupComponentStatus): string {
  if (status.lifecycle === 'ready') {
    return 'border-emerald-200 bg-emerald-50/95 text-emerald-600 shadow-emerald-200/50';
  }
  if (status.lifecycle === 'failed') {
    return 'border-rose-200 bg-rose-50/95 text-rose-600 shadow-rose-200/50';
  }
  if (status.lifecycle === 'pending') {
    return 'border-slate-200 bg-white/90 text-slate-400 shadow-slate-200/60';
  }
  return 'border-blue-200 bg-blue-50/95 text-blue-600 shadow-blue-200/70';
}
