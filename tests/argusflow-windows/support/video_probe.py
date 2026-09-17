"""监督独立录制进程，按秒记录内存、CPU和本地文件增长；超时只结束该子进程。"""
import argparse
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import subprocess
import time


class Memory(ctypes.Structure):
    _fields_ = [("cb", wintypes.DWORD), ("faults", wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in (
            "peak_working_set", "working_set", "peak_paged", "paged",
            "peak_nonpaged", "nonpaged", "pagefile", "peak_pagefile", "private")]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--seconds", type=int, default=600)
    parser.add_argument("--kill-after", type=int)
    parser.add_argument("--exe", default="target/debug/examples/record_video.exe")
    args = parser.parse_args()
    prefix = args.directory
    if prefix.exists():
        raise SystemExit("需要不存在的新目录")
    started = time.monotonic()
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(Memory), wintypes.DWORD]
    kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
    with open(str(prefix) + ".stdout", "w", encoding="utf-8") as out, \
         open(str(prefix) + ".stderr", "w", encoding="utf-8") as err, \
         open(str(prefix) + ".metrics.jsonl", "w", encoding="utf-8") as metrics:
        process = subprocess.Popen([args.exe, str(prefix), str(args.seconds)], stdout=out,
                                   stderr=err, creationflags=subprocess.CREATE_NO_WINDOW)
        last_cpu = 0
        last_elapsed = 0
        try:
            while process.poll() is None:
                elapsed = time.monotonic() - started
                memory = Memory()
                memory.cb = ctypes.sizeof(memory)
                if not psapi.GetProcessMemoryInfo(process._handle, ctypes.byref(memory), memory.cb):
                    if process.poll() is not None:
                        break
                    raise ctypes.WinError(ctypes.get_last_error())
                times = [wintypes.FILETIME() for _ in range(4)]
                if not kernel.GetProcessTimes(process._handle, *[ctypes.byref(t) for t in times]):
                    raise ctypes.WinError(ctypes.get_last_error())
                cpu = sum((t.dwHighDateTime << 32) | t.dwLowDateTime for t in times[2:]) / 10_000_000
                row = {"elapsed": elapsed, "rss": memory.working_set, "private": memory.private,
                       "cpu_core_percent": (cpu - last_cpu) / max(elapsed - last_elapsed, 0.001) * 100,
                       "cpu_seconds": cpu,
                       "video_bytes": (prefix / "screen.mp4").stat().st_size if (prefix / "screen.mp4").exists() else 0}
                metrics.write(json.dumps(row) + "\n")
                metrics.flush()
                last_cpu, last_elapsed = cpu, elapsed
                if args.kill_after and elapsed >= args.kill_after:
                    process.kill()
                    print("forced-stop", flush=True)
                    break
                if elapsed > args.seconds + 20:
                    process.kill()
                    raise TimeoutError("录制进程未在结束宽限期退出")
                if int(elapsed) % 60 == 0:
                    print(json.dumps(row), flush=True)
                time.sleep(1)
        finally:
            if process.poll() is None:
                process.kill()
            process.wait()
        print(json.dumps({"exit_code": process.returncode, "elapsed": time.monotonic() - started}), flush=True)
        if process.returncode and not args.kill_after:
            raise SystemExit(process.returncode)


if __name__ == "__main__":
    main()
