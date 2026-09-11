import { useState } from "react";
import {
  defaultValue,
  studio,
  type EditorTab,
  type Value,
} from "../../../features/workflow";
import { Button, Dialog, FormField } from "../../ui";
import { LiteralInput } from "../value-editor/LiteralInput";
export function RunInputsDialog({
  tab,
  onClose,
}: {
  readonly tab: EditorTab;
  readonly onClose: () => void;
}) {
  const [values, setValues] = useState<Readonly<Record<string, Value>>>(
    Object.fromEntries(
      Object.entries(tab.file.definition.inputs).map(([name, type]) => [
        name,
        defaultValue(type),
      ]),
    ),
  );
  const [invalid, setInvalid] = useState<readonly string[]>([]);
  return (
    <Dialog title="运行输入" onClose={onClose}>
      <div className="space-y-4">
        {Object.entries(tab.file.definition.inputs).map(([name, type]) => (
          <FormField key={name} label={name}>
            <LiteralInput
              type={type}
              value={values[name]}
              onChange={(value) => {
                setValues({ ...values, [name]: value });
                setInvalid(invalid.filter((item) => item !== name));
              }}
              onInvalid={() => setInvalid([...new Set([...invalid, name])])}
            />
          </FormField>
        ))}
      </div>
      {!!Object.keys(tab.file.definition.resources).length && (
        <p className="mt-4 text-xs text-danger">
          当前流程需要调用方提供资源，请通过引用它的父流程运行。
        </p>
      )}
      <div className="mt-6 flex justify-end gap-2">
        <Button onClick={onClose}>取消</Button>
        <Button
          variant="primary"
          disabled={
            invalid.length > 0 ||
            Object.keys(tab.file.definition.resources).length > 0
          }
          onClick={() => {
            onClose();
            void studio.safely(() => studio.run(values));
          }}
        >
          运行
        </Button>
      </div>
    </Dialog>
  );
}
