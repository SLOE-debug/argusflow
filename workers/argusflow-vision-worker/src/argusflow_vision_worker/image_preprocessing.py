"""Bounded desktop-text preprocessing with reversible OCR coordinates."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum

import cv2
import numpy


class ImagePreprocessingMode(str, Enum):
    """Image transformation requested by the strongly typed Rust OCR profile."""

    NONE = "none"
    ADAPTIVE_DESKTOP_TEXT = "adaptive_desktop_text"


@dataclass(frozen=True, slots=True)
class PreparedOcrImage:
    """Pixels sent to PaddleOCR plus the inverse geometry needed for its polygons."""

    pixels: numpy.ndarray
    input_width: int
    input_height: int
    output_width: int
    output_height: int
    contrast_enhanced: bool
    sharpened: bool

    def map_x_to_input(self, value: float) -> float:
        """Map an OCR x coordinate back into the original screenshot ROI."""

        return value * self.input_width / self.output_width

    def map_y_to_input(self, value: float) -> float:
        """Map an OCR y coordinate back into the original screenshot ROI."""

        return value * self.input_height / self.output_height

    def summary(self) -> dict[str, int | bool]:
        """Return the JSON schema consumed by ``OcrPreprocessingSummary``."""

        return {
            "input_width": self.input_width,
            "input_height": self.input_height,
            "output_width": self.output_width,
            "output_height": self.output_height,
            "contrast_enhanced": self.contrast_enhanced,
            "sharpened": self.sharpened,
        }


# Only compact query ROIs are enlarged. This cap prevents a full-window screenshot from
# multiplying inference cost and memory merely because one dimension is short.
_MAX_SOURCE_PIXELS = 300_000
_MAX_OUTPUT_PIXELS = 900_000
_TARGET_SHORT_SIDE = 160
_MAX_SCALE = 2.0
_LOW_CONTRAST_MIN_SPAN = 8.0
_LOW_CONTRAST_MAX_SPAN = 96.0


def prepare_ocr_image(rgb: numpy.ndarray, mode: ImagePreprocessingMode) -> PreparedOcrImage:
    """Adaptively upscale and enhance a small GUI ROI without changing its color semantics."""

    if rgb.ndim != 3 or rgb.shape[2] != 3 or rgb.dtype != numpy.uint8:
        raise ValueError("OCR preprocessing expects an RGB uint8 image")
    input_height, input_width = rgb.shape[:2]
    if input_width <= 0 or input_height <= 0:
        raise ValueError("OCR preprocessing requires a non-empty image")
    if mode is ImagePreprocessingMode.NONE:
        return _unchanged(rgb)

    source_pixels = input_width * input_height
    short_side = min(input_width, input_height)
    scale = 1.0
    if source_pixels <= _MAX_SOURCE_PIXELS and short_side < _TARGET_SHORT_SIDE:
        desired_scale = min(_MAX_SCALE, _TARGET_SHORT_SIDE / short_side)
        pixel_limited_scale = (_MAX_OUTPUT_PIXELS / source_pixels) ** 0.5
        scale = max(1.0, min(desired_scale, pixel_limited_scale))

    output_width = max(1, round(input_width * scale))
    output_height = max(1, round(input_height * scale))
    if output_width != input_width or output_height != input_height:
        prepared = cv2.resize(rgb, (output_width, output_height), interpolation=cv2.INTER_CUBIC)
    else:
        prepared = rgb

    # Percentiles make the decision insensitive to a few bright icons. CLAHE operates on
    # luminance only, retaining colored UI text while recovering anti-aliased dark-theme glyphs.
    luminance = cv2.cvtColor(prepared, cv2.COLOR_RGB2GRAY)
    low, high = numpy.percentile(luminance, (5.0, 95.0))
    contrast_span = float(high - low)
    contrast_enhanced = (
        source_pixels <= _MAX_SOURCE_PIXELS
        and _LOW_CONTRAST_MIN_SPAN <= contrast_span <= _LOW_CONTRAST_MAX_SPAN
    )
    if contrast_enhanced:
        lab = cv2.cvtColor(prepared, cv2.COLOR_RGB2LAB)
        lightness, channel_a, channel_b = cv2.split(lab)
        tile_size = max(4, min(8, short_side // 16 or 4))
        clahe = cv2.createCLAHE(clipLimit=1.6, tileGridSize=(tile_size, tile_size))
        prepared = cv2.cvtColor(
            cv2.merge((clahe.apply(lightness), channel_a, channel_b)),
            cv2.COLOR_LAB2RGB,
        )

    sharpened = scale > 1.0 or contrast_enhanced
    if sharpened:
        blurred = cv2.GaussianBlur(prepared, (0, 0), sigmaX=0.8, sigmaY=0.8)
        prepared = cv2.addWeighted(prepared, 1.3, blurred, -0.3, 0.0)

    return PreparedOcrImage(
        pixels=numpy.ascontiguousarray(prepared),
        input_width=input_width,
        input_height=input_height,
        output_width=output_width,
        output_height=output_height,
        contrast_enhanced=contrast_enhanced,
        sharpened=sharpened,
    )


def _unchanged(rgb: numpy.ndarray) -> PreparedOcrImage:
    """Describe an input that bypasses all preprocessing without copying its pixels."""

    height, width = rgb.shape[:2]
    return PreparedOcrImage(
        pixels=rgb,
        input_width=width,
        input_height=height,
        output_width=width,
        output_height=height,
        contrast_enhanced=False,
        sharpened=False,
    )
