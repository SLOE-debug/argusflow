import { afterEach, expect, it, vi } from "vitest";
import { WorkflowStudio } from "../../../src/features/workflow/studio/controller";
import { createWorkflow } from "../../../src/features/workflow/model/factory";
import type {
  DesktopApi,
  RunMessage,
} from "../../../src/features/workflow/api/desktop";

import { testDesktopApi as api } from "../support/desktopApi";
afterEach(() => vi.useRealTimers());
it("serializes overlapping flushes and saves the latest edit made during an in-flight save", async () => {
  vi.useFakeTimers();
  let release: (() => void) | undefined;
  const save = vi
    .fn<DesktopApi["save"]>()
    .mockImplementationOnce(async (file) => {
      await new Promise<void>((resolve) => {
        release = resolve;
      });
      return { file, revision: "one" };
    })
    .mockImplementation(async (file, revision) => {
      expect(revision).toBe("one");
      return { file, revision: "two" };
    });
  const studio = new WorkflowStudio(api({ save }));
  await studio.initializeWorkspace();
  studio.create();
  const a = studio.flushAll(),
    b = studio.flushAll();
  studio.edit((file) => ({
    ...file,
    definition: { ...file.definition, name: "latest" },
  }));
  const c = studio.flushAll();
  release!();
  await Promise.all([a, b, c]);
  expect(save).toHaveBeenCalledTimes(2);
  expect(studio.active?.status).toBe("saved");
  expect(studio.active?.file.definition.name).toBe("latest");
  expect(studio.active?.savedVersion).toBe(studio.active?.version);
});
it("preserves a conflict draft and blocks autosave until explicit recovery", async () => {
  vi.useFakeTimers();
  const save = vi
    .fn<DesktopApi["save"]>()
    .mockRejectedValue(new Error("conflict:external change"));
  const studio = new WorkflowStudio(api({ save }));
  await studio.initializeWorkspace();
  studio.create();
  await expect(studio.flushAll()).rejects.toThrow("conflict:");
  studio.edit((file) => ({
    ...file,
    definition: { ...file.definition, name: "still here" },
  }));
  await vi.advanceTimersByTimeAsync(900);
  expect(save).toHaveBeenCalledTimes(1);
  expect(studio.active?.file.definition.name).toBe("still here");
  expect(studio.active?.status).toBe("conflict");
});
it("keeps paste and deletion as one undo transaction and redo restores owned scopes", async () => {
  vi.useFakeTimers();
  const studio = new WorkflowStudio(api());
  await studio.initializeWorkspace();
  studio.create(createWorkflow());
  studio.add("block", { x: 10, y: 20 });
  expect(studio.active?.file.definition.scopes).toHaveLength(2);
  studio.duplicate();
  expect(studio.active?.file.definition.scopes).toHaveLength(3);
  studio.undo();
  expect(studio.active?.file.definition.scopes).toHaveLength(2);
  studio.redo();
  expect(studio.active?.file.definition.scopes).toHaveLength(3);
  await studio.flushAll();
});
it("freezes edits before save and validation while starting a run", async () => {
  vi.useFakeTimers();
  let release: (() => void) | undefined;
  const validate = vi.fn(async () => {
    await new Promise<void>((resolve) => {
      release = resolve;
    });
    return [];
  });
  const start = vi.fn<DesktopApi["start"]>(async () => "run");
  const studio = new WorkflowStudio(api({ validate, start }));
  await studio.initializeWorkspace();
  studio.create();
  const run = studio.run({});
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  expect(studio.readonly).toBe(true);
  studio.edit((file) => ({
    ...file,
    definition: { ...file.definition, name: "not allowed" },
  }));
  expect(studio.active?.file.definition.name).not.toBe("not allowed");
  await vi.waitFor(() => expect(release).toBeTypeOf("function"));
  release!();
  await run;
  expect(start).toHaveBeenCalledTimes(1);
});

it("saving a conflict as a copy preserves the draft and unblocks subsequent saves", async () => {
  vi.useFakeTimers();
  const save = vi
    .fn<DesktopApi["save"]>()
    .mockRejectedValueOnce(new Error("conflict:external change"))
    .mockImplementation(async (file) => ({ file, revision: "copy" }));
  const studio = new WorkflowStudio(api({ save }));
  await studio.initializeWorkspace();
  studio.create();
  const original = studio.active!.file;
  await expect(studio.flushAll()).rejects.toThrow("conflict");
  studio.saveCopy();
  await studio.flushAll();
  expect(studio.active!.file.id).not.toBe(original.id);
  expect(studio.active!.file.definition.scopes).toEqual(
    original.definition.scopes,
  );
  expect(studio.store.getState().tabs[original.id]).toBeUndefined();
  expect(studio.active!.status).toBe("saved");
  expect(save).toHaveBeenCalledTimes(2);
});

it("subscriptions retain ordered logs and reveal a final failure after switching panels", async () => {
  let receive: ((message: RunMessage) => void) | undefined;
  const studio = new WorkflowStudio(
    api({
      subscribe: async (handler) => {
        receive = handler;
      },
    }),
  );
  await studio.subscribe();
  const snapshot = {
    id: "run",
    workflow: "flow",
    documents: ["flow"],
    status: "running" as const,
    logs: [],
    omitted: 0,
    outputs: {},
    errors: [],
  };
  receive!({ type: "snapshot", snapshot });
  const entry = {
    sequence: "1",
    elapsed_ms: "0",
    kind: "node_completed",
    level: "info",
    message: "done",
    path: [],
  };
  receive!({ type: "log", id: "run", entry });
  receive!({ type: "log", id: "run", entry });
  receive!({ type: "log", id: "old", entry: { ...entry, sequence: "2" } });
  expect(studio.store.getState().run!.logs).toHaveLength(1);
  studio.panel("data");
  studio.toggleDock();
  receive!({
    type: "snapshot",
    snapshot: {
      ...snapshot,
      status: "failed",
      logs: [entry],
      errors: ["failure"],
    },
  });
  expect(studio.store.getState().dock).toBe("logs");
  expect(studio.store.getState().dockOpen).toBe(true);
  expect(studio.store.getState().run!.status).toBe("failed");
});
