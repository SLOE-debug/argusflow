/** 录制 BMP 的纯像素差分，不调用视觉模型，不根据任务文字选区域。 */
export function decodeBmp(bytes) {
  if (
    bytes.toString("ascii", 0, 2) !== "BM" ||
    bytes.readUInt16LE(28) !== 24 ||
    bytes.readUInt32LE(30) !== 0
  )
    throw Error("只支持录制器的 24 位无压缩 BMP");
  const width = bytes.readInt32LE(18),
    signedHeight = bytes.readInt32LE(22);
  const height = Math.abs(signedHeight),
    stride = (width * 3 + 3) & ~3,
    offset = bytes.readUInt32LE(10);
  if (
    width <= 0 ||
    height <= 0 ||
    width * height > 16_777_216 ||
    offset + stride * height > bytes.length
  )
    throw Error("BMP 尺寸无效");
  return {
    width,
    height,
    bytes,
    pixel(x, y) {
      return offset + (signedHeight < 0 ? y : height - y - 1) * stride + x * 3;
    },
  };
}

/** 扫描授权区域；边界保留 12px 上下文，不变时不制造变化区域。 */
export function changedBounds(before, after, scope) {
  if (before.width !== after.width || before.height !== after.height)
    return { unavailable: "窗口尺寸不同，不能直接像素差分" };
  const [x, y, w, h] = scope;
  if (
    [x, y, w, h].some((n) => !Number.isInteger(n)) ||
    x < 0 ||
    y < 0 ||
    w < 1 ||
    h < 1 ||
    x + w > after.width ||
    y + h > after.height
  )
    throw Error("差分区域越界");
  let left = x + w,
    top = y + h,
    right = x - 1,
    bottom = y - 1,
    changed = 0;
  for (let row = y; row < y + h; row++)
    for (let col = x; col < x + w; col++) {
      const a = before.pixel(col, row),
        b = after.pixel(col, row);
      if (
        [0, 1, 2].some(
          (c) => Math.abs(before.bytes[a + c] - after.bytes[b + c]) > 20,
        )
      ) {
        left = Math.min(left, col);
        right = Math.max(right, col);
        top = Math.min(top, row);
        bottom = Math.max(bottom, row);
        changed++;
      }
    }
  if (!changed) return { unchanged: true };
  left = Math.max(x, left - 12);
  top = Math.max(y, top - 12);
  right = Math.min(x + w - 1, right + 12);
  bottom = Math.min(y + h - 1, bottom + 12);
  return {
    crop: [left, top, right - left + 1, bottom - top + 1],
    changed_pixels: changed,
  };
}

/** 按需纳入与差分相交的 OCR 行，仍裁在授权区域内，不扩成整屏。 */
export function textContext(crop, blocks, scope) {
  let [left, top, width, height] = crop;
  let right = left + width,
    bottom = top + height;
  for (const block of blocks) {
    const [x, y, w, h] = block.rect;
    if (
      block.confidence < 0.7 ||
      x >= crop[0] + crop[2] ||
      x + w <= crop[0] ||
      y >= crop[1] + crop[3] ||
      y + h <= crop[1]
    )
      continue;
    left = Math.min(left, x - 12);
    top = Math.min(top, y - 8);
    right = Math.max(right, x + w + 12);
    bottom = Math.max(bottom, y + h + 8);
  }
  left = Math.max(scope[0], left);
  top = Math.max(scope[1], top);
  right = Math.min(scope[0] + scope[2], right);
  bottom = Math.min(scope[1] + scope[3], bottom);
  return [
    Math.floor(left),
    Math.floor(top),
    Math.ceil(right - left),
    Math.ceil(bottom - top),
  ];
}
