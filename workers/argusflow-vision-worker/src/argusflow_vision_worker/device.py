"""Paddle inference device selection with measured CUDA fallback."""

from __future__ import annotations

import os
from dataclasses import dataclass
from enum import Enum
from typing import Any


class DeviceKind(str, Enum):
    """Finite inference device families understood by Paddle and the wire protocol."""

    CPU = "cpu"
    CUDA = "cuda"


@dataclass(frozen=True, slots=True)
class InferenceDevice:
    """A validated Paddle device and its protocol representation."""

    kind: DeviceKind
    index: int | None = None

    @property
    def paddle_name(self) -> str:
        """Return the device name accepted by PaddleOCR."""

        return "cpu" if self.kind is DeviceKind.CPU else f"gpu:{self.index}"

    def as_wire(self) -> dict[str, Any]:
        """Serialize the device to the Rust tagged enum contract."""

        if self.kind is DeviceKind.CPU:
            return {"kind": "cpu"}
        return {"kind": "cuda", "index": self.index}


@dataclass(frozen=True, slots=True)
class DeviceSelection:
    """The usable device plus an optional automatic-fallback reason."""

    device: InferenceDevice
    degradation_reason: str | None


def select_inference_device() -> DeviceSelection:
    """Prefer CUDA when requested or automatic, then verify it with a real tensor op."""

    import paddle

    preference = os.environ.get("ARGUSFLOW_PADDLE_DEVICE", "auto").strip().lower()
    if preference == "cpu":
        paddle.set_device("cpu")
        return DeviceSelection(InferenceDevice(DeviceKind.CPU), None)
    if preference not in {"auto", "gpu", "gpu:0"}:
        raise ValueError(f"unsupported ARGUSFLOW_PADDLE_DEVICE value: {preference}")

    try:
        if not paddle.device.is_compiled_with_cuda():
            raise RuntimeError("the installed Paddle runtime was built without CUDA")
        device_count = int(paddle.device.cuda.device_count())
        if device_count < 1:
            raise RuntimeError("Paddle did not find a visible CUDA device")
        paddle.set_device("gpu:0")
        # A tensor allocation catches missing CUDA DLLs and driver/runtime mismatches before
        # expensive OCR model construction starts.
        probe = paddle.zeros([1], dtype="float32")
        _ = float(probe.numpy()[0])
        return DeviceSelection(InferenceDevice(DeviceKind.CUDA, 0), None)
    except Exception as error:
        paddle.set_device("cpu")
        return DeviceSelection(
            InferenceDevice(DeviceKind.CPU),
            f"CUDA initialization failed; switched to CPU: {error}",
        )
