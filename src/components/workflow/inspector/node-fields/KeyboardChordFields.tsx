import type {
  KeyChord,
  KeyboardKey,
  KeyboardModifier,
} from '../../../../features/workflow';
import { Checkbox, Input, Select } from '../../../ui';
import { INSPECTOR_HELP_CLASS_NAME, InspectorField } from '../InspectorControls';

/** 属性面板可编辑的主键类别。 */
type KeyboardKeyKind = KeyboardKey['type'];

const KEY_OPTIONS = [
  { value: 'enter', label: 'Enter' },
  { value: 'escape', label: 'Esc' },
  { value: 'tab', label: 'Tab' },
  { value: 'character', label: '字母或数字' },
] as const;

const MODIFIER_OPTIONS = [
  { value: 'control', label: 'Ctrl' },
  { value: 'alt', label: 'Alt' },
  { value: 'shift', label: 'Shift' },
] as const;

/** 编辑一次完整组合键，不把虚拟键码暴露到工作流。 */
export function KeyboardChordFields({
  chord,
  onChange,
}: Readonly<{
  chord: KeyChord;
  onChange: (chord: KeyChord) => void;
}>) {
  return (
    <div className="flex flex-col gap-2.5 rounded-md border border-slate-200 bg-slate-50/70 p-2.5">
      <InspectorField label="按键">
        <Select<KeyboardKeyKind>
          value={chord.key.type}
          options={KEY_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(type) => onChange({
            ...chord,
            key: createKeyboardKey(type),
          })}
        />
      </InspectorField>
      {chord.key.type === 'character' ? (
        <InspectorField label="字母或数字">
          <Input
            aria-label="组合键字母或数字"
            value={chord.key.value}
            maxLength={1}
            containerClassName="border-slate-300 bg-white"
            onChange={(event) => onChange({
              ...chord,
              key: { type: 'character', value: event.target.value },
            })}
          />
        </InspectorField>
      ) : null}
      <div className="flex flex-wrap gap-x-3 gap-y-1.5">
        {MODIFIER_OPTIONS.map((option) => (
          <label
            key={option.value}
            className="flex items-center gap-1.5 text-[11px] text-slate-700"
          >
            <Checkbox
              aria-label={`${option.label} 修饰键`}
              checked={chord.modifiers.includes(option.value)}
              onChange={(event) => onChange({
                ...chord,
                modifiers: setModifier(
                  chord.modifiers,
                  option.value,
                  event.target.checked,
                ),
              })}
            />
            {option.label}
          </label>
        ))}
      </div>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        组合键会发送到指定应用的当前焦点。
      </p>
    </div>
  );
}

/** 为主键类别创建字段完整的判别联合。 */
function createKeyboardKey(type: KeyboardKeyKind): KeyboardKey {
  return type === 'character' ? { type, value: 'a' } : { type };
}

/** 添加或移除一个修饰键，并保持界面声明顺序。 */
function setModifier(
  current: readonly KeyboardModifier[],
  modifier: KeyboardModifier,
  enabled: boolean,
): KeyboardModifier[] {
  const selected = new Set(current);
  if (enabled) selected.add(modifier);
  else selected.delete(modifier);
  return MODIFIER_OPTIONS
    .map(({ value }) => value)
    .filter((value) => selected.has(value));
}
