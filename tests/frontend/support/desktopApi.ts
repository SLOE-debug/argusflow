import { vi } from "vitest";
import type { DesktopApi } from "../../../src/features/workflow/api/desktop";

/** 桌面传输替身，未配置的加载明确失败。 */
export function testDesktopApi(
  overrides: Partial<DesktopApi> = {},
): DesktopApi {
  return {
    initializeWorkspace: async () => ({
      path: "workspace",
      documents: [],
    }),
    listDocuments: async () => [],
    load: async () => {
      throw new Error("missing");
    },
    save: vi.fn(async (file) => ({ file, revision: crypto.randomUUID() })),
    deleteDocument: vi.fn(async () => {}),
    validate: async () => [],
    start: async () => "run",
    stop: async () => {},
    capabilities: async () => [],
    subscribe: async () => {},
    copy: async () => {},
    paste: async () => null,
    parseClipboard: async () => {
      throw new Error("invalid clipboard");
    },
    exportLog: async () => {},
    describeTask: async () => ({}),
    ...overrides,
  };
}
