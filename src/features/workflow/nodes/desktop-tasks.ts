import type { TaskSpec } from "./catalog";
import { BOOL, TEXT } from "../model/expressions";
const WINDOW = "automation.window";
export const DESKTOP_TASKS: readonly TaskSpec[] = [
  {
    id: "window.ocr",
    title: "建立窗口 OCR 范围",
    category: "桌面自动化",
    description: "加载 OCR 模型，识别指定窗口的可见区域",
    inputs: {},
    outputs: {},
    resources: { window: WINDOW },
    creates: { source: "automation.query_source" },
    config: [],
  },
  {
    id: "file.create_new",
    title: "新建空文件",
    category: "数据",
    description: "预留新文件；路径已存在时停止，避免覆盖",
    inputs: { path: TEXT },
    outputs: {},
    resources: {},
    creates: {},
    config: [],
  },
  {
    id: "file.wait_text",
    title: "等待文件保存",
    category: "数据",
    description: "等待文件的 UTF-8 内容与预期完全一致，超时则停止",
    inputs: { path: TEXT, expected: TEXT },
    outputs: { equal: BOOL },
    resources: {},
    creates: {},
    config: [],
  },
];
