/** 只读证据帧生成缩略图/前后变化裁剪图，禁止任意路径和任意屏幕读取。 */
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { decodeBmp, changedBounds, textContext } from "./pixels.mjs";
const execute = promisify(execFile);

export class EvidenceImages {
  constructor(recording, output) {
    this.recording = recording;
    this.output = output;
    this.count = 0;
    this.pixels = 0;
  }
  async render(samples, crop, prefix, maxWidth, maxPixels) {
    if (this.count + samples.length > 7) throw Error("已达到 7 张图像预算");
    const scale = Math.min(
      1,
      maxWidth / crop[2],
      Math.sqrt(maxPixels / (crop[2] * crop[3])),
    );
    const width = Math.max(1, Math.floor(crop[2] * scale)),
      height = Math.max(1, Math.floor(crop[3] * scale));
    if (this.pixels + width * height * samples.length > 1_600_000)
      throw Error("累计图像像素预算耗尽");
    await mkdir(this.output, { recursive: true });
    const jobs = samples.map((s, index) => {
      if (!/^frames\/\d+\.bmp$/.test(s.image))
        throw Error("帧文件路径不在白名单");
      return {
        input: resolve(this.recording.directory, s.image),
        output: resolve(this.output, `${prefix}-${index}.png`),
        crop,
        width,
        height,
      };
    });
    const manifest = resolve(this.output, `${prefix}.json`);
    await writeFile(manifest, JSON.stringify(jobs));
    await execute(
      "powershell.exe",
      [
        "-NoProfile",
        "-File",
        fileURLToPath(new URL("./render.ps1", import.meta.url)),
        "-Manifest",
        manifest,
      ],
      { windowsHide: true, timeout: 15000 },
    );
    this.count += samples.length;
    this.pixels += width * height * samples.length;
    return Promise.all(
      jobs.map(async (job, i) => ({
        file: job.output,
        sample_id: `sample-${samples[i].id}`,
        crop,
        width,
        height,
        data: (await readFile(job.output)).toString("base64"),
      })),
    );
  }
  async overview() {
    const sample = this.recording.rawSamples.find(
      (s) => s.window?.class === "Chrome_WidgetWin_1",
    );
    if (!sample) throw Error("缺少授权测试浏览器开场帧");
    return this.render(
      [sample],
      [0, 0, sample.bounds[2], sample.bounds[3]],
      "overview",
      768,
      400_000,
    );
  }
  async change(id, context = "change") {
    if (!["change", "text_line"].includes(context))
      throw Error("未知图像上下文");
    const observation = this.recording.details.get(id);
    if (!observation || !id.startsWith("sample-"))
      throw Error("view_change 需要时间线中的 sample ID");
    const samples = this.recording.rawSamples;
    const index = samples.findIndex((s) => `sample-${s.id}` === id);
    const after = samples[index];
    const before = samples
      .slice(0, index)
      .findLast((s) => s.window?.handle === after.window.handle);
    if (!before) return { unavailable: "此窗口没有更早的帧", images: [] };
    const read = async (s) => {
      if (!/^frames\/\d+\.bmp$/.test(s.image)) throw Error("未知帧路径");
      return decodeBmp(
        await readFile(resolve(this.recording.directory, s.image)),
      );
    };
    const diff = changedBounds(
      await read(before),
      await read(after),
      observation.crop,
    );
    if (!diff.crop)
      return { ...diff, images: [], before: before.id, after: after.id };
    const crop =
      context === "text_line"
        ? textContext(
            diff.crop,
            [...before.ocr, ...after.ocr],
            observation.crop,
          )
        : diff.crop;
    const images = await this.render(
      [before, after],
      crop,
      `change-${after.id}-${context}`,
      640,
      180_000,
    );
    return {
      ...diff,
      crop,
      context,
      before: before.id,
      after: after.id,
      notes:
        "同窗口前后帧差分及所请求的局部上下文；不同步 UIA；授权区域外像素不上传。",
      images,
    };
  }
}
