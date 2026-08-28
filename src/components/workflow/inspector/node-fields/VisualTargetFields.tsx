import type {
  NormalizedRect,
  UiOperation,
} from '../../../../features/workflow';
import { changeTargetLocator } from '../../../../features/workflow';
import { Input, Select } from '../../../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from '../InspectorControls';
import { ValueExprFields } from './ValueExprFields';

type VisualTargetFieldsProps = Readonly<{
  /** 当前 UI 节点的完整语义操作契约。 */
  operation: UiOperation;
  /** 当前视觉目标的持久化定位契约。 */
  locator: Extract<UiOperation['target']['locator'], { type: 'visual' }>;
  /** 写回字段完整的新操作。 */
  onChange: (operation: UiOperation) => void;
}>;

/** 编辑显式 OCR/视觉文字目标及其归一化识别区域。 */
export function VisualTargetFields({
  operation,
  locator,
  onChange,
}: VisualTargetFieldsProps) {
  const visualQuery = locator.query;
  const regionMode = visualQuery.region ? 'custom' : 'full';
  const region = visualQuery.region ?? FULL_VISUAL_REGION;
  const updateQuery = (patch: Partial<typeof visualQuery>) => onChange(changeTargetLocator(operation, {
    type: 'visual',
    query: { ...visualQuery, ...patch },
  }));
  const updateRegion = (axis: keyof NormalizedRect, value: number) => {
    updateQuery({ region: updateNormalizedRegion(region, axis, value) });
  };
  return (
    <>
      <ValueExprFields
        value={visualQuery.text}
        literalLabel="目标文字"
        expressionLocation={{ type: 'ui_visual_text' }}
        onChange={(value) => updateQuery({ text: value })}
      />
      <InspectorField label="匹配方式">
        <Select<'exact' | 'contains'>
          value={visualQuery.exact ? 'exact' : 'contains'}
          options={VISUAL_MATCH_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(mode) => updateQuery({ exact: mode === 'exact' })}
        />
      </InspectorField>
      <InspectorField label="识别范围">
        <Select<'full' | 'custom'>
          aria-label="视觉识别范围"
          value={regionMode}
          options={VISUAL_REGION_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(mode) => updateQuery({
            region: mode === 'custom' ? region : null,
          })}
        />
      </InspectorField>
      {regionMode === 'custom' ? (
        <div className="grid grid-cols-2 gap-2 rounded-md border border-slate-200 bg-slate-50/60 p-2.5">
          <NormalizedRegionField
            label="左侧 X"
            ariaLabel="视觉区域 X"
            value={region.x}
            onChange={(value) => updateRegion('x', value)}
          />
          <NormalizedRegionField
            label="顶部 Y"
            ariaLabel="视觉区域 Y"
            value={region.y}
            onChange={(value) => updateRegion('y', value)}
          />
          <NormalizedRegionField
            label="宽度"
            ariaLabel="视觉区域宽度"
            value={region.width}
            onChange={(value) => updateRegion('width', value)}
          />
          <NormalizedRegionField
            label="高度"
            ariaLabel="视觉区域高度"
            value={region.height}
            onChange={(value) => updateRegion('height', value)}
          />
        </div>
      ) : null}
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        文字会在稳定画面中严格匹配；自定义范围按应用窗口的比例计算，可减少重复命中。
      </p>
    </>
  );
}

/** 视觉查询默认覆盖整个应用窗口。 */
const FULL_VISUAL_REGION: NormalizedRect = {
  x: 0,
  y: 0,
  width: 1,
  height: 1,
};

/** 视觉区域选择使用全窗口或显式归一化区域。 */
const VISUAL_REGION_OPTIONS = [
  { value: 'full', label: '整个应用窗口' },
  { value: 'custom', label: '自定义范围' },
] as const;

/** 视觉匹配使用完全相等或包含文字两种明确规则。 */
const VISUAL_MATCH_OPTIONS = [
  { value: 'exact', label: '完全匹配' },
  { value: 'contains', label: '包含文字' },
] as const;

type NormalizedRegionFieldProps = Readonly<{
  label: string;
  ariaLabel: string;
  value: number;
  onChange: (value: number) => void;
}>;

/** 编辑单个归一化区域分量；公共 Input 负责基础控件样式与可访问性。 */
function NormalizedRegionField({
  label,
  ariaLabel,
  value,
  onChange,
}: NormalizedRegionFieldProps) {
  return (
    <label className="flex min-w-0 flex-col gap-1 text-[10px] text-slate-600">
      {label}
      <Input
        aria-label={ariaLabel}
        type="number"
        min={0}
        max={1}
        step={0.01}
        value={value}
        containerClassName="border-slate-300 bg-white"
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  );
}

/** 修改区域时同步收紧相邻边界，避免把无效矩形交给 Runtime。 */
function updateNormalizedRegion(
  region: NormalizedRect,
  axis: keyof NormalizedRect,
  value: number,
): NormalizedRect {
  const nextValue = Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0;
  switch (axis) {
    case 'x':
      return { ...region, x: Math.min(nextValue, 1 - region.width) };
    case 'y':
      return { ...region, y: Math.min(nextValue, 1 - region.height) };
    case 'width':
      return { ...region, width: Math.min(1 - region.x, Math.max(0.01, nextValue)) };
    case 'height':
      return { ...region, height: Math.min(1 - region.y, Math.max(0.01, nextValue)) };
  }
}
