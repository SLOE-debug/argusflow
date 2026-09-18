import { TagSelect } from "../../ui";
/** 与后端组合键编译器的支持范围一致；持久化仍使用明确的按键序列。 */
const KEYS = [
  "Control",
  "Shift",
  "Alt",
  "Enter",
  "Escape",
  "Tab",
  "Home",
  "End",
  "Backspace",
  "Delete",
  "Space",
  "Left",
  "Right",
  "Up",
  "Down",
  ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ",
];
export function KeyChordField({
  value,
  onChange,
}: {
  readonly value: string;
  readonly onChange: (value: string) => void;
}) {
  return (
    <TagSelect
      label="组合键"
      values={value ? value.split("+") : []}
      options={KEYS.map((key) => ({ value: key, label: key }))}
      limit={8}
      onChange={(keys) => onChange(keys.join("+"))}
    />
  );
}
