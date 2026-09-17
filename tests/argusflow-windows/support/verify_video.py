"""顺序解码并核对逐帧PTS索引；仅用于测试，FFmpeg不是产品运行依赖。"""
import argparse
from fractions import Fraction
import json
from pathlib import Path
import re
import subprocess
import imageio_ffmpeg


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--incomplete", action="store_true")
    parser.add_argument("--colors", action="store_true")
    args = parser.parse_args()
    executable = imageio_ffmpeg.get_ffmpeg_exe()
    common = [executable, "-v", "error", "-copyts", "-i", str(args.directory / "screen.mp4"),
              "-fps_mode", "passthrough", "-enc_time_base", "-1"]
    count = 0
    maximum_error = Fraction(0)
    previous = None
    with open(args.directory / "frames.jsonl", encoding="utf-8") as index, \
         open(args.directory / "decode-errors.log", "w+", encoding="utf-8") as errors:
        process = subprocess.Popen(common + ["-f", "framemd5", "-"], stdout=subprocess.PIPE,
                                   stderr=errors, text=True, encoding="utf-8")
        try:
            for line in process.stdout:
                if line.startswith("#tb 0:"):
                    timebase = Fraction(line.split(":", 1)[1].strip())
                if line.startswith("#"):
                    continue
                fields = line.split(",")
                if len(fields) != 6:
                    continue
                actual = int(fields[2]) * timebase
                record = json.loads(next(index))
                expected = Fraction(record["frame"]["pts_100ns"], 10_000_000)
                error = abs(actual - expected)
                assert error <= timebase, (count, actual, expected, timebase)
                assert previous is None or actual > previous, (count, previous, actual)
                maximum_error = max(maximum_error, error)
                previous = actual
                count += 1
            assert process.wait(timeout=60) == 0
            errors.seek(0)
            assert not errors.read().strip(), "解码器报告错误"
            if not args.incomplete:
                assert next(index, None) is None, "已提交索引包含未解码的视频帧"
            assert count > 0
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
    if args.colors:
        pixels = subprocess.check_output(common + ["-vf", "scale=1:1", "-pix_fmt", "rgb24", "-f", "rawvideo", "-"])
        assert len(pixels) == 9, len(pixels)
        for i in range(3):
            rgb = pixels[i * 3:i * 3 + 3]
            assert rgb[i] >= 240 and max(rgb[j] for j in range(3) if j != i) <= 15, list(rgb)
    summary = {"decoded_frames": count, "max_pts_error_us": float(maximum_error * 1_000_000),
               "incomplete_prefix": args.incomplete, "synthetic_rgb_verified": args.colors}
    (args.directory / "decode-verification.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
