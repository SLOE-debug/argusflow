import type { BrowserSpec } from '../../features/workflow/contracts';
import { Input } from '../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from './InspectorControls';

type BrowserNodeFieldsProps = Readonly<{
  /** 当前隔离 Chromium 启动契约。 */
  spec: BrowserSpec;
  /** 写回字段完整的新契约。 */
  onChange: (spec: BrowserSpec) => void;
}>;

/** 编辑浏览器 EXE、初始 URL 和有界启动等待。 */
export function BrowserNodeFields({ spec, onChange }: BrowserNodeFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="浏览器 EXE">
        <Input
          aria-label="浏览器可执行文件"
          value={spec.executable_path}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...spec,
            executable_path: event.target.value,
          })}
        />
      </InspectorField>
      <InspectorField label="初始地址">
        <Input
          aria-label="浏览器初始地址"
          value={spec.initial_url}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...spec,
            initial_url: event.target.value,
          })}
        />
      </InspectorField>
      <InspectorField label="启动超时毫秒">
        <Input
          aria-label="浏览器启动超时毫秒"
          type="number"
          min={100}
          max={60_000}
          value={spec.launch_timeout_ms}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...spec,
            launch_timeout_ms: Number(event.target.value),
          })}
        />
      </InspectorField>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        每次运行使用隔离临时 profile 和随机调试端口，结束时自动关闭并清理。
      </p>
    </div>
  );
}
