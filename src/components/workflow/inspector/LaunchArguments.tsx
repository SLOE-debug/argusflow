import { type Expr, type SymbolValue } from "../../../features/workflow";
import { ValueField } from "../value-editor/ValueField";
/** 参数原样传给任意可执行程序，不推测程序支持的启动开关。 */
export function LaunchArguments({
  value,
  symbols,
  pending,
  onChange,
  onInvalid,
}: {
  readonly value: Expr;
  readonly symbols: readonly SymbolValue[];
  readonly pending?: string;
  readonly onChange: (value: Expr) => void;
  readonly onInvalid: (source: string) => void;
}) {
  return (
    <div className="space-y-2">
      <ValueField
        value={value}
        type={{ type: "list", of: { type: "text" } }}
        symbols={symbols}
        pending={pending}
        onChange={onChange}
        onInvalid={onInvalid}
      />
      <p className="text-xs text-muted">
        每项输入一个参数或文件路径，按顺序传入程序，无需额外加引号。
      </p>
    </div>
  );
}
