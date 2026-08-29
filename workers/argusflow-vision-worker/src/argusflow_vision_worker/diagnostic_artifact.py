"""Lossless diagnostic artifacts derived from the exact ndarray used for inference."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass

import cv2
import numpy


@dataclass(frozen=True, slots=True)
class EncodedModelInput:
    """PNG body and typed control metadata returned to the Rust Host."""

    body: bytes
    width: int
    height: int
    sha256: str

    def metadata(self) -> dict[str, str | int]:
        """Return the v4 binary artifact control contract."""

        return {
            "kind": "model_input",
            "encoding": "png",
            "width": self.width,
            "height": self.height,
            "body_length": len(self.body),
            "sha256": self.sha256,
        }


def encode_exact_model_input(pixels: numpy.ndarray) -> EncodedModelInput:
    """Encode RGB uint8 pixels so a standards decoder reconstructs the predict ndarray exactly."""

    if pixels.ndim != 3 or pixels.shape[2] != 3 or pixels.dtype != numpy.uint8:
        raise ValueError("model-input diagnostics require an RGB uint8 image")
    # OpenCV accepts BGR input and writes a standards-compliant RGB PNG. Explicit conversion is
    # required because `pixels` is the RGB ndarray passed directly to PaddleOCR.
    bgr = cv2.cvtColor(pixels, cv2.COLOR_RGB2BGR)
    encoded, buffer = cv2.imencode(".png", bgr, [cv2.IMWRITE_PNG_COMPRESSION, 3])
    if not encoded:
        raise RuntimeError("failed to encode exact OCR model input as PNG")
    body = bytes(buffer)
    height, width = pixels.shape[:2]
    return EncodedModelInput(
        body=body,
        width=width,
        height=height,
        sha256=hashlib.sha256(body).hexdigest(),
    )
