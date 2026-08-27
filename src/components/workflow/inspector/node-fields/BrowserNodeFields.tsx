import type { BrowserSpec } from '../../../../features/workflow';
import { Input } from '../../../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from '../InspectorControls';

type BrowserNodeFieldsProps = Readonly<{
  /** 当前隔离 Chromium 启动契约。 */
  spec: BrowserSpec;
  /** 写回字段完整的新契约。 */
  onChange: (spec: BrowserSpec) => void;
}>;

/** 编辑只负责会话获取的浏览器 EXE 和有界启动等待。 */
export function BrowserNodeFields({ spec, onChange }: BrowserNodeFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="浏览器程序">
        <Input
          aria-label="浏览器程序路径"
          value={spec.executable_path}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...spec,
            executable_path: event.target.value,
          })}
        />
      </InspectorField>
      <InspectorField label="启动超时（毫秒）">
        <Input
          aria-label="浏览器启动超时"
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
        这里先打开浏览器；网址请填写在后面的“打开网页”节点中。
      </p>
    </div>
  );
}
