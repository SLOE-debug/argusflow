"""Direct PaddleOCR Result mapping without JSON serialization overhead."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from .image_preprocessing import PreparedOcrImage
from .protocol import ProtocolError


def items_from_prediction(
    prediction: Any,
    roi: Any,
    prepared: PreparedOcrImage,
    minimum_score: float,
) -> list[dict[str, Any]]:
    """Align direct Result fields and map model coordinates back to the captured frame."""

    if not isinstance(prediction, Mapping):
        raise RuntimeError("PaddleOCR prediction is not a mapping result")
    if not isinstance(roi, dict):
        raise ProtocolError("invalid_roi", "OCR request ROI is missing")
    offset_x = int(roi["x"])
    offset_y = int(roi["y"])
    texts = list(prediction.get("rec_texts", []))
    scores = list(prediction.get("rec_scores", []))
    polygons = list(prediction.get("rec_polys", []))
    if not polygons:
        boxes = list(prediction.get("rec_boxes", []))
        polygons = [
            [[box[0], box[1]], [box[2], box[1]], [box[2], box[3]], [box[0], box[3]]]
            for box in boxes
            if len(box) >= 4
        ]
    items: list[dict[str, Any]] = []
    for index, text in enumerate(texts):
        polygon = polygons[index] if index < len(polygons) else []
        points = [
            [
                prepared.map_x_to_input(float(point[0])) + offset_x,
                prepared.map_y_to_input(float(point[1])) + offset_y,
            ]
            for point in polygon
            if len(point) >= 2
        ]
        score = float(scores[index]) if index < len(scores) else 0.0
        if not points or score < minimum_score:
            continue
        items.append(
            {
                "raw_text": str(text),
                "confidence": max(0.0, min(1.0, score)),
                "polygon": [{"x": point[0], "y": point[1]} for point in points],
            }
        )
    return items
