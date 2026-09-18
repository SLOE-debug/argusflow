import { useStore } from "zustand";
import { studio } from "../../../features/workflow";
import { Tabs } from "../../ui";
/** 文档导航只映射工作流状态，键盘与标签样式由 UI 库维护。 */
export function WorkflowTabs() {
  const state = useStore(studio.store),
    items = Object.values(state.tabs);
  if (!items.length) return null;
  return (
    <Tabs
      label="工作流标签"
      className="ml-2 shrink self-stretch"
      value={state.active}
      items={items.map((item) => ({
        value: item.file.id,
        label: item.file.definition.name,
        title: item.file.definition.name,
        marker: item.version !== item.savedVersion ? "未保存" : undefined,
      }))}
      onChange={(id) => {
        void studio.safely(() => studio.open(id));
      }}
      onClose={(id) => {
        void studio.safely(() => studio.close(id));
      }}
    />
  );
}
