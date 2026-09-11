import { useEffect, useState } from "react";
import {
  studio,
  taskSpec,
  type TaskDefinition,
  type ValueType,
} from "../../../features/workflow";
/** AQL 参数的权威类型来自同一个 Rust 编译器，引用模式下也不猜测类型。 */
export function useTaskInputs(
  task: TaskDefinition | null,
): Readonly<Record<string, ValueType>> {
  const [fields, setFields] = useState<Readonly<Record<string, ValueType>>>({});
  const query = task?.config.query;
  const id = task?.type_id;
  useEffect(() => {
    let alive = true;
    if (task && typeof query === "string" && query) {
      void studio.api
        .describeTask(task.type_id, task.config)
        .then((fields) => {
          if (alive) setFields(fields);
        })
        .catch(() => {
          if (alive) setFields({});
        });
    } else setFields({});
    return () => {
      alive = false;
    };
  }, [id, query]);
  return { ...(task ? taskSpec(task.type_id)?.inputs : {}), ...fields };
}
