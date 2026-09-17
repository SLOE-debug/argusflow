import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { StructureDetails } from "../../../src/components/recorder/StructureDetails";
import type { RecordingRecord } from "../../../src/features/recorder/model";

const record: RecordingRecord = {
  id: 4,
  written_qpc: 130,
  data: {
    Structure: {
      raw: [1],
      source: "Uia",
      relation: "After",
      from_qpc: 120,
      through_qpc: 130,
      identity: { pid: "123" },
      properties: {
        name: "搜索",
        automation_id: "SearchBox",
        enabled: true,
        role: 50004,
      },
      ancestors: [{ name: "桌面窗口" }],
      bounds: [10, 20, 100, 40],
      truncated: true,
      stale: false,
      sensitive: false,
      target_confirmed: false,
      scope: "事件后焦点观察",
    },
  },
};
it("常用属性显示中文，技术属性默认折叠", () => {
  render(<StructureDetails records={[record]} />);
  expect(screen.getByText("SearchBox")).toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "搜索 · 输入框" }),
  ).toBeInTheDocument();
  expect(screen.getByText("是")).toBeVisible();
  expect(screen.getByText("SearchBox")).not.toBeVisible();
  expect(screen.getByText("10, 20, 100, 40")).toBeInTheDocument();
  expect(screen.getByText("只读取了部分控件信息。")).toBeInTheDocument();
  expect(screen.getByText("所属窗口与容器")).toBeInTheDocument();
  expect(screen.queryByText(/已比较|变化区域/)).not.toBeInTheDocument();
});

it("合并相同快照，但保留内容发生变化的快照", () => {
  if (!("Structure" in record.data)) throw new Error("缺少控件快照");
  const duplicate: RecordingRecord = {
    ...record,
    id: 5,
    written_qpc: 150,
    data: {
      Structure: {
        ...record.data.Structure,
        raw: [2],
        from_qpc: 140,
        through_qpc: 150,
      },
    },
  };
  const changed: RecordingRecord = {
    ...record,
    id: 6,
    data: {
      Structure: {
        ...record.data.Structure,
        properties: {
          ...record.data.Structure.properties,
          observed_value: "新的内容",
        },
      },
    },
  };
  render(<StructureDetails records={[record, duplicate, changed]} />);
  expect(
    screen.getAllByRole("heading", { name: "搜索 · 输入框" }),
  ).toHaveLength(2);
  expect(screen.getByText("2 次读取结果相同，已合并显示")).toBeVisible();
  expect(screen.getByText("记录编号：4、5")).toBeInTheDocument();
  expect(screen.getByText("新的内容")).toBeVisible();
});

it("说明对象不匹配的原因，不再误称信息过期", () => {
  if (!("Structure" in record.data)) throw new Error("缺少控件快照");
  render(
    <StructureDetails
      records={[
        {
          ...record,
          data: { Structure: { ...record.data.Structure, stale: true } },
        },
      ]}
    />,
  );
  expect(screen.getByText(/来自另一个进程/)).toBeVisible();
  expect(screen.queryByText(/已过期/)).not.toBeInTheDocument();
});
