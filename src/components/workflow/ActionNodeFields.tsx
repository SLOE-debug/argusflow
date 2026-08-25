import type {
  AutomationAction,
  AutomationActionKind,
  BackendPreference,
  TargetLocatorKind,
} from '../../features/workflow/contracts';
import {
  changeAutomationActionKind,
  changeBackendPreference,
  changeSetValueText,
  changeTargetLocator,
  changeTargetLocatorKind,
} from '../../features/workflow/workflowAction';
import { Input, Select, Textarea } from '../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from './InspectorControls';
import { AqlEditor } from './AqlEditor';

type ActionNodeFieldsProps = Readonly<{
  /** 当前 Action 节点的完整动作契约。 */
  action: AutomationAction;
  /** 写回字段完整的新动作。 */
  onChange: (action: AutomationAction) => void;
}>;

/** Action、定位方式和后端偏好的强类型选项。 */
const ACTION_KIND_OPTIONS = [
  { value: 'click', label: '点击元素' },
  { value: 'set_value', label: '填写文本' },
] as const;

const LOCATOR_KIND_OPTIONS = [
  { value: 'query', label: 'AQL 语义查询' },
  { value: 'visual', label: '视觉文字' },
  { value: 'coordinate', label: '屏幕坐标' },
] as const;

const BACKEND_OPTIONS = [
  { value: 'auto', label: '自动规划' },
  { value: 'windows_uia', label: 'Windows UIA' },
  { value: 'browser_cdp', label: 'Browser CDP' },
] as const;

const VISUAL_MATCH_OPTIONS = [
  { value: 'exact', label: '完全相等' },
  { value: 'contains', label: '允许包含' },
] as const;

/** 编辑 Action 的动作类型、参数和三种目标定位契约。 */
export function ActionNodeFields({ action, onChange }: ActionNodeFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="执行动作">
        <Select<AutomationActionKind>
          value={action.type}
          options={ACTION_KIND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(kind) => onChange(changeAutomationActionKind(action, kind))}
        />
      </InspectorField>
      {action.type === 'set_value' ? (
        <InspectorField label="填写内容">
          <Textarea
            aria-label="填写内容"
            className="h-[76px] resize-y border-slate-300 bg-white leading-[18px]"
            value={action.value}
            onChange={(event) => onChange(changeSetValueText(action, event.target.value))}
          />
        </InspectorField>
      ) : null}
      <InspectorField label="定位方式">
        <Select<TargetLocatorKind>
          value={action.target.locator.type}
          options={LOCATOR_KIND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(kind) => onChange(changeTargetLocatorKind(action, kind))}
        />
      </InspectorField>
      {action.target.locator.type === 'query' ? (
        <>
          <InspectorField label="后端偏好">
            <Select<BackendPreference>
              value={action.target.backend_preference}
              options={BACKEND_OPTIONS}
              containerClassName="border-slate-300 bg-white"
              onValueChange={(preference) => (
                onChange(changeBackendPreference(action, preference))
              )}
            />
          </InspectorField>
          <AqlEditor
            query={action.target.locator.query}
            backendPreference={action.target.backend_preference}
            onChange={(query) => onChange(changeTargetLocator(action, {
              type: 'query',
              query,
            }))}
          />
        </>
      ) : null}
      {action.target.locator.type === 'visual' ? (
        <VisualTargetFields
          action={action}
          locator={action.target.locator}
          onChange={onChange}
        />
      ) : null}
      {action.target.locator.type === 'coordinate' ? (
        <CoordinateTargetFields
          action={action}
          locator={action.target.locator}
          onChange={onChange}
        />
      ) : null}
    </div>
  );
}

/** 编辑显式 OCR/视觉文字目标。 */
function VisualTargetFields({
  action,
  locator,
  onChange,
}: Readonly<{
  action: AutomationAction;
  locator: Extract<AutomationAction['target']['locator'], { type: 'visual' }>;
  onChange: (action: AutomationAction) => void;
}>) {
  const visualQuery = locator.query;

  return (
    <>
      <InspectorField label="目标文字">
        <Input
          aria-label="视觉目标文字"
          value={visualQuery.text}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange(changeTargetLocator(action, {
            type: 'visual',
            query: { ...visualQuery, text: event.target.value },
          }))}
        />
      </InspectorField>
      <InspectorField label="匹配方式">
        <Select<'exact' | 'contains'>
          value={visualQuery.exact ? 'exact' : 'contains'}
          options={VISUAL_MATCH_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(mode) => onChange(changeTargetLocator(action, {
            type: 'visual',
            query: { ...visualQuery, exact: mode === 'exact' },
          }))}
        />
      </InspectorField>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        视觉文字由 OCR 或 GUI grounding 后端定位，执行后端固定由 planner 自动选择。
      </p>
    </>
  );
}

/** 编辑 Windows 虚拟屏幕中的物理像素坐标。 */
function CoordinateTargetFields({
  action,
  locator,
  onChange,
}: Readonly<{
  action: AutomationAction;
  locator: Extract<AutomationAction['target']['locator'], { type: 'coordinate' }>;
  onChange: (action: AutomationAction) => void;
}>) {
  const point = locator.point;
  const updatePoint = (axis: 'x' | 'y', value: number) => {
    onChange(changeTargetLocator(action, {
      type: 'coordinate',
      point: { ...point, [axis]: value },
    }));
  };

  return (
    <>
      <InspectorField label="屏幕 X">
        <Input
          aria-label="屏幕 X 坐标"
          type="number"
          value={point.x}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => updatePoint('x', Number(event.target.value))}
        />
      </InspectorField>
      <InspectorField label="屏幕 Y">
        <Input
          aria-label="屏幕 Y 坐标"
          type="number"
          value={point.y}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => updatePoint('y', Number(event.target.value))}
        />
      </InspectorField>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        坐标使用 Windows 虚拟屏幕物理像素，适合无法提供语义树的最终兜底场景。
      </p>
    </>
  );
}
