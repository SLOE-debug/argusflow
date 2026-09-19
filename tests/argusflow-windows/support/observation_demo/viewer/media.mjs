import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

/** 仅转换既有帧，保留原始像素；不重新截图。 */
export async function embedFrames(directory, samples, viewerDirectory) {
  const destination = resolve(directory, "viewer-assets");
  await mkdir(destination, { recursive: true });
  const paths = [
    ...new Set(samples.map((sample) => sample.image).filter(Boolean)),
  ];
  const jobs = [];
  for (const path of paths) {
    if (!/^frames\/\d+\.bmp$/.test(path))
      throw new Error(`非预期帧路径：${path}`);
    const bytes = await readFile(resolve(directory, path));
    const width = Math.abs(bytes.readInt32LE(18)),
      height = Math.abs(bytes.readInt32LE(22));
    jobs.push({
      input: resolve(directory, path),
      output: resolve(
        destination,
        path.split("/").at(-1).replace(".bmp", ".png"),
      ),
      crop: [0, 0, width, height],
      width,
      height,
    });
  }
  const manifest = resolve(destination, "manifest.json");
  await writeFile(manifest, JSON.stringify(jobs));
  await promisify(execFile)(
    "powershell.exe",
    [
      "-NoProfile",
      "-File",
      resolve(viewerDirectory, "../conversation/render.ps1"),
      "-Manifest",
      manifest,
    ],
    { windowsHide: true, timeout: 120000 },
  );
  return Object.fromEntries(
    await Promise.all(
      jobs.map(async (job, index) => [
        paths[index],
        `data:image/png;base64,${(await readFile(job.output)).toString("base64")}`,
      ]),
    ),
  );
}
