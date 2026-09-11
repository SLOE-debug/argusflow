import {
  Braces,
  Clock3,
  GitBranch,
  Globe2,
  MousePointer2,
  Repeat2,
  Search,
  Workflow,
  AppWindow,
  ArrowRightFromLine,
  CircleStop,
  Box,
  Variable,
  ListFilter,
} from "lucide-react";
/** 图标目录沿用实验分支的 Lucide 系列，节点语义由调用方传入。 */
export function NodeIcon({
  kind,
  size = 16,
}: {
  readonly kind: string;
  readonly size?: number;
}) {
  const Icon =
    kind.startsWith("browser.") || kind === "source.dom"
      ? Globe2
      : kind.startsWith("application.") ||
          kind.startsWith("window.") ||
          kind === "source.uia"
        ? AppWindow
        : kind === "aql.click" || kind === "aql.type_text"
          ? MousePointer2
          : kind.startsWith("aql.")
            ? Search
            : kind === "while" || kind === "for_each"
              ? Repeat2
              : kind === "if" || kind === "switch"
                ? GitBranch
                : kind === "call_workflow"
                  ? Workflow
                  : kind === "wait"
                    ? Clock3
                    : kind === "let"
                      ? Variable
                      : kind === "assign"
                        ? Braces
                        : kind === "return"
                          ? ArrowRightFromLine
                          : kind === "fail"
                            ? CircleStop
                            : kind === "release"
                              ? Box
                              : ListFilter;
  return <Icon size={size} strokeWidth={1.7} aria-hidden="true" />;
}
