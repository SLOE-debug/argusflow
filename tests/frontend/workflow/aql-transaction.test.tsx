import {
  fireEvent,
  render,
  screen,
  waitFor,
  act,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useStore } from "zustand";
import { AqlDock } from "../../../src/components/workflow/workspace/AqlDock";
import {
  studio,
  createWorkflow,
  type EditorTab,
  type ValueType,
} from "../../../src/features/workflow";
import {
  addNode,
  updateNode,
} from "../../../src/features/workflow/model/graph";
import { INITIAL_STATE } from "../../../src/features/workflow/studio/state";

vi.mock("../../../src/components/editor/monaco/CodeEditor", () => ({
  CodeEditor: ({
    source,
    onChange,
  }: {
    source: string;
    onChange: (value: string) => void;
  }) => (
    <textarea
      aria-label="query"
      value={source}
      onChange={(event) => onChange(event.target.value)}
    />
  ),
}));
vi.mock("../../../src/features/aql", () => ({
  loadLanguageService: async () => ({
    importEnglish: (source: string) => source,
    inspect: (source: string) => ({ english: source, diagnostics: [] }),
  }),
}));
afterEach(() => vi.restoreAllMocks());
function Editor({ nodeId }: { readonly nodeId: string }) {
  const state = useStore(studio.store);
  return <AqlDock tab={state.tabs[state.active!]} nodeId={nodeId} />;
}
it("an old asynchronous AQL application cannot clear a newer query draft", async () => {
  const empty = createWorkflow(),
    added = addNode(empty, empty.definition.root, "aql.exists", { x: 0, y: 0 });
  const file = updateNode(added.file, added.id, (node) =>
    node.action.kind === "task"
      ? {
          ...node,
          action: {
            ...node.action,
            task: { ...node.action.task, config: { query: "old" } },
          },
        }
      : node,
  );
  const tab: EditorTab = {
    file,
    version: 0,
    savedVersion: 0,
    status: "saved",
    revision: "one",
    past: [],
    future: [],
    scope: file.definition.root,
    selected: [added.id],
    selectedEdge: null,
    viewport: { x: 0, y: 0, zoom: 1 },
  };
  studio.store.setState({
    ...INITIAL_STATE,
    tabs: { [file.id]: tab },
    active: file.id,
  });
  vi.spyOn(studio.api, "save").mockImplementation(async (file) => ({
    file,
    revision: "two",
  }));
  let release:
    ((value: Readonly<Record<string, ValueType>>) => void) | undefined;
  vi.spyOn(studio.api, "describeTask").mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  render(<Editor nodeId={added.id} />);
  await waitFor(() =>
    expect(screen.getByLabelText("query")).toHaveValue("old"),
  );
  fireEvent.click(screen.getByRole("button", { name: "应用查询" }));
  fireEvent.change(screen.getByLabelText("query"), {
    target: { value: "new unsaved query" },
  });
  await act(async () => release!({}));
  expect(studio.active!.file.editor.drafts[added.id + ":aql"]).toBe(
    "new unsaved query",
  );
  await studio.flushAll();
});

function installQuery() {
  const empty = createWorkflow();
  const added = addNode(empty, empty.definition.root, "aql.exists", {
    x: 0,
    y: 0,
  });
  const file = updateNode(added.file, added.id, (node) =>
    node.action.kind === "task"
      ? {
          ...node,
          action: {
            ...node.action,
            task: {
              ...node.action.task,
              config: { query: "saved" },
              inputs: {
                name: {
                  kind: "literal",
                  value_type: { type: "text" },
                  value: { type: "text", value: "原参数" },
                },
              },
            },
          },
        }
      : node,
  );
  const tab: EditorTab = {
    file,
    version: 0,
    savedVersion: 0,
    status: "saved",
    revision: "one",
    past: [],
    future: [],
    scope: file.definition.root,
    selected: [added.id],
    selectedEdge: null,
    viewport: { x: 0, y: 0, zoom: 1 },
  };
  studio.store.setState({
    ...INITIAL_STATE,
    tabs: { [file.id]: tab },
    active: file.id,
  });
  vi.spyOn(studio.api, "save").mockImplementation(async (file) => ({
    file,
    revision: "two",
  }));
  return added.id;
}

it("reopening a saved query restores parameters without applying it again", async () => {
  const id = installQuery();
  vi.spyOn(studio.api, "describeTask").mockResolvedValue({
    name: { type: "text" },
  });
  render(<Editor nodeId={id} />);
  await waitFor(() =>
    expect(screen.getByLabelText("固定值")).toHaveValue("原参数"),
  );
  expect(studio.active!.past).toHaveLength(0);
});

it("query drafts follow undo and redo without a stale local editor copy", async () => {
  const id = installQuery();
  vi.spyOn(studio.api, "describeTask").mockResolvedValue({});
  render(<Editor nodeId={id} />);
  await waitFor(() =>
    expect(screen.getByLabelText("query")).toHaveValue("saved"),
  );
  fireEvent.change(screen.getByLabelText("query"), {
    target: { value: "edited" },
  });
  act(() => studio.undo());
  expect(screen.getByLabelText("query")).toHaveValue("saved");
  act(() => studio.redo());
  expect(screen.getByLabelText("query")).toHaveValue("edited");
  await studio.flushAll();
});

it("applying a changed parameter list removes only deleted input drafts in one transaction", async () => {
  const id = installQuery();
  vi.spyOn(studio.api, "describeTask").mockImplementation(
    async (_type, config): Promise<Readonly<Record<string, ValueType>>> =>
      config.query === "updated"
        ? { remaining: { type: "int" } }
        : { name: { type: "text" } },
  );
  studio.draft(id, "input.name", "unfinished removed field");
  studio.draft(id, "input.remaining", "-");
  studio.draft(id, "timeout", "bad timeout");
  render(<Editor nodeId={id} />);
  await waitFor(() =>
    expect(screen.getByLabelText("query")).toHaveValue("saved"),
  );
  fireEvent.change(screen.getByLabelText("query"), {
    target: { value: "updated" },
  });
  const before = studio.active!.past.length;
  fireEvent.click(screen.getByRole("button", { name: "应用查询" }));
  await waitFor(() =>
    expect(studio.active!.file.editor.drafts[id + ":aql"]).toBeUndefined(),
  );
  expect(studio.active!.past).toHaveLength(before + 1);
  expect(studio.active!.file.editor.drafts).toEqual({
    [id + ":input.remaining"]: "-",
    [id + ":timeout"]: "bad timeout",
  });
  act(() => studio.undo());
  expect(studio.active!.file.editor.drafts[id + ":input.name"]).toBe(
    "unfinished removed field",
  );
  expect(screen.getByLabelText("query")).toHaveValue("updated");
  await studio.flushAll();
});

it("query application does not overwrite configuration changed during the request", async () => {
  const id = installQuery();
  let release:
    ((fields: Readonly<Record<string, ValueType>>) => void) | undefined;
  vi.spyOn(studio.api, "describeTask")
    .mockResolvedValueOnce({})
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          release = resolve;
        }),
    )
    .mockResolvedValue({});
  render(<Editor nodeId={id} />);
  await waitFor(() =>
    expect(screen.getByLabelText("query")).toHaveValue("saved"),
  );
  fireEvent.change(screen.getByLabelText("query"), {
    target: { value: "pending" },
  });
  fireEvent.click(screen.getByRole("button", { name: "应用查询" }));
  act(() =>
    studio.changeNode(id, (node) =>
      node.action.kind === "task"
        ? {
            ...node,
            action: {
              ...node.action,
              task: {
                ...node.action.task,
                config: { ...node.action.task.config, present: false },
              },
            },
          }
        : node,
    ),
  );
  await act(async () => release!({}));
  expect(studio.active!.file.editor.drafts[id + ":aql"]).toBe("pending");
  await studio.flushAll();
});
