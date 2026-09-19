import test from "node:test";
import assert from "node:assert/strict";
import {
  decodeBmp,
  changedBounds,
  textContext,
} from "../support/observation_demo/conversation/pixels.mjs";
import { dispatchTool } from "../support/observation_demo/conversation/tools.mjs";

test("文字上下文扩展保留相交整行且不越过授权区域", () => {
  assert.deepEqual(
    textContext(
      [160, 101, 29, 48],
      [{ rect: [20, 110, 155, 24], confidence: 0.99 }],
      [8, 93, 300, 100],
    ),
    [8, 101, 181, 48],
  );
});
test("文字上下文不包含不相交的其他文字", () => {
  assert.deepEqual(
    textContext(
      [160, 101, 29, 48],
      [{ rect: [20, 200, 155, 24], confidence: 0.99 }],
      [8, 93, 300, 200],
    ),
    [160, 101, 29, 48],
  );
});

function bitmap(width = 40, height = 30) {
  const stride = (width * 3 + 3) & ~3;
  const data = Buffer.alloc(54 + stride * height);
  data.write("BM");
  data.writeUInt32LE(54, 10);
  data.writeInt32LE(width, 18);
  data.writeInt32LE(-height, 22);
  data.writeUInt16LE(24, 28);
  return decodeBmp(data);
}

test("变化裁剪只涵盖实际变化和有限上下文", () => {
  const before = bitmap(),
    after = bitmap();
  after.bytes[after.pixel(20, 15)] = 255;
  assert.deepEqual(changedBounds(before, after, [0, 0, 40, 30]), {
    crop: [8, 3, 25, 25],
    changed_pixels: 1,
  });
});
test("区域外变化不得出现在上传裁剪图", () => {
  const before = bitmap(),
    after = bitmap();
  after.bytes[after.pixel(1, 1)] = 255;
  assert.deepEqual(changedBounds(before, after, [10, 10, 20, 20]), {
    unchanged: true,
  });
});
test("尺寸变化不冒充同坐标像素变化", () => {
  assert.ok(changedBounds(bitmap(), bitmap(41), [0, 0, 40, 30]).unavailable);
});
test("越界裁剪与截断位图明确失败", () => {
  assert.throws(() => changedBounds(bitmap(), bitmap(), [-1, 0, 40, 30]));
  assert.throws(() => decodeBmp(bitmap().bytes.subarray(0, 55)));
});
test("证据工具分页返回，不丢弃待查询 ID", async () => {
  const ids = Array.from({ length: 9 }, (_, i) => `key-${i}`);
  const recording = {
    details: new Map(ids.map((id) => [id, { keys: ["Control", "C"] }])),
  };
  const result = await dispatchTool(recording, null, {
    function: { name: "inspect_evidence", arguments: JSON.stringify({ ids }) },
  });
  assert.equal(result.facts.length, 6);
  assert.deepEqual(result.remaining_ids, ids.slice(6));
});
test("模型不能用工具访问文件或指定任意截图区域", async () => {
  await assert.rejects(
    dispatchTool({}, null, {
      function: { name: "read_file", arguments: '{"path":".env"}' },
    }),
  );
  await assert.rejects(
    dispatchTool({}, null, {
      function: {
        name: "view_change",
        arguments: '{"sample_id":"sample-1","path":".env"}',
      },
    }),
  );
});
