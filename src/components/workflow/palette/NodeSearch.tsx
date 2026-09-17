import { useRef, useState } from "react";
import { Search } from "lucide-react";
import { NODE_CATALOG } from "../../../features/workflow";
import { Button, Dialog, Input } from "../../ui";
import { NodeIcon } from "../presentation/NodeIcon";
/** 右键、Tab 和拖线落点共用同一个搜索插入器。 */
export function NodeSearch({
  onPick,
  onClose,
}: {
  readonly onPick: (kind: string) => void;
  readonly onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  /** -1 表示尚未通过方向键或指针选定结果，打开和改词均不预选首项。 */
  const [index, setIndex] = useState(-1);
  const input = useRef<HTMLInputElement>(null);
  const matches = NODE_CATALOG.filter((item) =>
    (item.title + item.id + item.category)
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  return (
    <Dialog title="添加节点" onClose={onClose} initialFocus={input}>
      <div className="mb-3">
        <Input
          leading={<Search size={14} />}
          ref={input}
          aria-label="搜索节点"
          placeholder="搜索名称或动作，例如“点击”"
          className="w-full"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setIndex(-1);
          }}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing) return;
            if (event.key === "ArrowDown") {
              event.preventDefault();
              setIndex(Math.min(matches.length - 1, index + 1));
            }
            if (event.key === "ArrowUp") {
              event.preventDefault();
              setIndex(index < 0 ? matches.length - 1 : Math.max(0, index - 1));
            }
            if (event.key === "Enter" && matches[index])
              onPick(matches[index].id);
          }}
        />
      </div>
      <div
        className="max-h-80 space-y-0.5 overflow-auto"
        onPointerLeave={() => setIndex(-1)}
      >
        {matches.map((item, position) => (
          <Button
            key={item.id}
            variant="ghost"
            data-highlighted={position === index}
            className={
              "h-11 w-full justify-start gap-3 transition-none " +
              (position === index ? "bg-accent-soft" : "")
            }
            onClick={() => onPick(item.id)}
            onPointerMove={() => setIndex(position)}
            onFocus={() => setIndex(position)}
          >
            <span className="shrink-0">
              <NodeIcon kind={item.id} />
            </span>
            <span className="min-w-0 text-left">
              <span className="block">{item.title}</span>
              <span className="block truncate text-[10px] font-normal text-muted">
                {item.description}
              </span>
            </span>
            <span className="ml-auto text-[10px] text-muted">
              {item.category}
            </span>
          </Button>
        ))}
        {!matches.length && (
          <p className="py-6 text-center text-xs text-muted">
            没有匹配节点，试试其他关键词。
          </p>
        )}
      </div>
    </Dialog>
  );
}
