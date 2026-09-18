import { studio, type EditorTab } from "../../../features/workflow";
import { useState } from "react";
import { Tabs } from "../../ui";
import { WorkflowInputs } from "./WorkflowInputs";
import { WorkflowResults } from "./WorkflowResults";

/** 编排运行前填写的内容与运行后显示的结果。 */
export function DataPanel({ tab }: { readonly tab: EditorTab }) {
  const [page, setPage] = useState<"inputs" | "outputs">("inputs");
  return (
    <div className="flex h-full flex-col">
      <div className="shrink-0 border-b border-line px-4 py-2">
        <Tabs
          label="流程设置"
          value={page}
          onChange={setPage}
          items={[
            { value: "inputs", label: "开始前填写", title: "开始前填写" },
            { value: "outputs", label: "结果设置", title: "结果设置" },
          ]}
        />
      </div>
      <fieldset
        disabled={studio.readonly}
        className="min-h-0 flex-1 overflow-auto"
      >
        <div className="mx-auto w-full max-w-5xl">
          {page === "inputs" ? (
            <WorkflowInputs tab={tab} />
          ) : (
            <WorkflowResults key={tab.file.id} tab={tab} />
          )}
        </div>
      </fieldset>
    </div>
  );
}
