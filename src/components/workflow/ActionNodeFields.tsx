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
  { value: 'click', label: '点击' },
  { value: 'set_value', label: '输入文字' },
] as const;

const LOCATOR_KIND_OPTIONS = [
  { value: 'application_query', label: '应用内查找（启动/唤醒）' },
  { value: 'query', label: '当前窗口查找（AQL）' },
  { value: 'visual', label: '按画面文字查找' },
  { value: 'coordinate', label: '按屏幕位置' },
] as const;

const BACKEND_OPTIONS = [
  { value: 'auto', label: '自动选择（推荐）' },
  { value: 'windows_uia', label: 'Windows UIA' },
  { value: 'browser_cdp', label: 'Browser CDP' },
] as const;

const VISUAL_MATCH_OPTIONS = [
  { value: 'exact', label: '完全相等' },
  { value: 'contains', label: '允许包含' },
] as const;

const WINDOW_TITLE_MATCH_OPTIONS = [
  { value: 'equal', label: '完全相等' },
  { value: 'contains', label: '允许包含' },
] as const;

/** 编辑 Action 的动作类型、参数和三种目标定位契约。 */
export function ActionNodeFields({ action, onChange }: ActionNodeFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="操作">
        <Select<AutomationActionKind>
          value={action.type}
          options={ACTION_KIND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(kind) => onChange(changeAutomationActionKind(action, kind))}
        />
      </InspectorField>
      {action.type === 'set_value' ? (
        <InspectorField label="输入内容">
          <Textarea
            aria-label="输入内容"
            className="h-[76px] resize-y border-slate-300 bg-white leading-[18px]"
            value={action.value}
            onChange={(event) => onChange(changeSetValueText(action, event.target.value))}
          />
        </InspectorField>
      ) : null}
      <InspectorField label="查找目标">
        <Select<TargetLocatorKind>
          value={action.target.locator.type}
          options={LOCATOR_KIND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(kind) => onChange(changeTargetLocatorKind(action, kind))}
        />
      </InspectorField>
      {action.target.locator.type === 'query'
        || action.target.locator.type === 'application_query' ? (
          <QueryTargetFields
            action={action}
            locator={action.target.locator}
            onChange={onChange}
          />
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

/** 编辑前台窗口或显式应用作用域中的 AQL 目标。 */
function QueryTargetFields({
  action,
  locator,
  onChange,
}: Readonly<{
  action: AutomationAction;
  locator: Extract<
    AutomationAction['target']['locator'],
    { type: 'query' | 'application_query' }
  >;
  onChange: (action: AutomationAction) => void;
}>) {
  /** 在保持查询和作用域判别字段的同时更新应用契约。 */
  const updateApplication = (
    application: Extract<typeof locator, { type: 'application_query' }>['application'],
  ) => {
    if (locator.type === 'application_query') {
      onChange(changeTargetLocator(action, { ...locator, application }));
    }
  };

  return (
    <>
      {locator.type === 'application_query' ? (
        <div className="flex flex-col gap-2.5 rounded-md border border-blue-100 bg-blue-50/40 p-2.5">
          <InspectorField label="应用 EXE">
            <Input
              aria-label="应用 EXE 绝对路径"
              value={locator.application.executable_path}
              containerClassName="border-slate-300 bg-white"
              onChange={(event) => updateApplication({
                ...locator.application,
                executable_path: event.target.value,
              })}
            />
          </InspectorField>
          <InspectorField label="窗口标题">
            <Input
              aria-label="应用窗口标题"
              value={locator.application.window_title.value}
              containerClassName="border-slate-300 bg-white"
              onChange={(event) => updateApplication({
                ...locator.application,
                window_title: {
                  ...locator.application.window_title,
                  value: event.target.value,
                },
              })}
            />
          </InspectorField>
          <InspectorField label="标题匹配">
            <Select<'equal' | 'contains'>
              value={locator.application.window_title.type}
              options={WINDOW_TITLE_MATCH_OPTIONS}
              containerClassName="border-slate-300 bg-white"
              onValueChange={(type) => updateApplication({
                ...locator.application,
                window_title: {
                  type,
                  value: locator.application.window_title.value,
                },
              })}
            />
          </InspectorField>
          <InspectorField label="启动参数">
            <Textarea
              aria-label="应用启动参数"
              className="h-[58px] resize-y border-slate-300 bg-white leading-[18px]"
              value={locator.application.arguments.join('\n')}
              onChange={(event) => updateApplication({
                ...locator.application,
                arguments: event.target.value
                  .split('\n')
                  .map((argument) => argument.trim())
                  .filter(Boolean),
              })}
            />
          </InspectorField>
          <InspectorField label="启动超时">
            <Input
              aria-label="应用启动超时毫秒"
              type="number"
              min={100}
              max={60_000}
              value={locator.application.launch_timeout_ms}
              containerClassName="border-slate-300 bg-white"
              onChange={(event) => updateApplication({
                ...locator.application,
                launch_timeout_ms: Number(event.target.value),
              })}
            />
          </InspectorField>
          <p className={INSPECTOR_HELP_CLASS_NAME}>
            已运行时恢复并激活唯一匹配窗口；未运行时直接启动 EXE 并等待窗口。
          </p>
        </div>
      ) : null}
      <details className="rounded-md border border-slate-200 bg-slate-50/70 px-2.5 py-2">
        <summary className="cursor-pointer select-none text-[10px] font-medium text-slate-600">
          高级设置
        </summary>
        <div className="mt-2">
          {locator.type === 'query' ? (
            <InspectorField label="执行方式约束">
              <Select<BackendPreference>
                value={action.target.backend_preference}
                options={BACKEND_OPTIONS}
                containerClassName="border-slate-300 bg-white"
                onValueChange={(preference) => (
                  onChange(changeBackendPreference(action, preference))
                )}
              />
            </InspectorField>
          ) : (
            <p className={INSPECTOR_HELP_CLASS_NAME}>应用生命周期目标固定使用 Windows UIA。</p>
          )}
        </div>
      </details>
      <AqlEditor
        query={locator.query}
        target={action.target}
        onChange={(query) => onChange(changeTargetLocator(action, {
          ...locator,
          query,
        }))}
      />
    </>
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
