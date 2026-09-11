import { useEffect, useMemo, useRef, useState } from "react";
import { loadLanguageService, type LanguageService } from "../../aql";
import type { EditorTab } from "./state";
import type { TaskDefinition, ValueType } from "../model/contracts";
import { nodeById } from "../model/graph";
import { applyQuery } from "../nodes/query";
import { studio } from "./controller";

/** 查询草稿从文档派生，撤销、重做和重挂载使用同一个版本。 */
export function useQueryEditor(tab: EditorTab, nodeId: string) {
  const node = nodeById(tab.file, nodeId);
  const task = node?.action.kind === "task" ? node.action.task : null;
  const [service, setService] = useState<LanguageService | null>(null);
  const [error, setError] = useState("");
  const [composing, setComposing] = useState(false);
  const [applying, setApplying] = useState(false);
  /** 请求版本同时受输入、配置改变和组件卸载约束。 */
  const version = useRef(0);
  const [description, setDescription] = useState<{
    readonly config: TaskDefinition["config"];
    readonly fields: Readonly<Record<string, ValueType>>;
  } | null>(null);
  useEffect(() => {
    let alive = true;
    void loadLanguageService()
      .then((loaded) => {
        if (alive) setService(loaded);
      })
      .catch((failure) => {
        if (alive) setError(String(failure));
      });
    return () => {
      alive = false;
      version.current++;
    };
  }, [tab.file.id, nodeId]);

  const config = task?.config;
  const typeId = task?.type_id;
  useEffect(() => {
    let alive = true;
    if (config && typeId && typeof config.query === "string" && config.query) {
      void studio.api
        .describeTask(typeId, config)
        .then((fields) => {
          if (alive) setDescription({ config, fields });
        })
        .catch((failure) => {
          if (alive) setError(String(failure));
        });
    }
    return () => {
      alive = false;
    };
  }, [config, typeId]);

  const pending = tab.file.editor.drafts[nodeId + ":aql"];
  const source = useMemo(
    () => pending ?? service?.importEnglish(String(config?.query ?? "")) ?? "",
    [pending, service, config?.query],
  );
  const analysis = useMemo(
    () => (service && !composing ? service.inspect(source) : null),
    [service, source, composing],
  );
  const parameters =
    description?.config === config ? (description?.fields ?? {}) : {};
  const change = (value: string) => {
    if (studio.readonly) return;
    version.current++;
    setApplying(false);
    studio.draft(nodeId, "aql", value);
  };
  const apply = async () => {
    const english = analysis?.english;
    if (!english || !task || studio.readonly) return;
    const request = ++version.current;
    setApplying(true);
    try {
      const fields = await studio.api.describeTask(task.type_id, {
        ...task.config,
        query: english,
      });
      const current = studio.active;
      const currentNode = current ? nodeById(current.file, nodeId) : undefined;
      // 比较原始草稿和配置，防止撤销、切换标签或修改其他配置后旧请求重新写入。
      if (
        version.current !== request ||
        current?.file.id !== tab.file.id ||
        currentNode?.action.kind !== "task" ||
        currentNode.action.task.config !== task.config ||
        current.file.editor.drafts[nodeId + ":aql"] !== pending ||
        studio.readonly
      )
        return;
      studio.edit((file) => applyQuery(file, nodeId, english, fields));
      setError("");
    } catch (failure) {
      if (version.current === request) setError(String(failure));
    } finally {
      if (version.current === request) setApplying(false);
    }
  };
  return {
    service,
    error,
    setError,
    source,
    parameters,
    analysis,
    applying,
    change,
    apply,
    setComposing,
  };
}
