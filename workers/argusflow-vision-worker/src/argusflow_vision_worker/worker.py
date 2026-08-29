"""PaddleOCR 3.7.0 adapter and bounded Named Pipe request loop."""

from __future__ import annotations

import importlib.metadata
import os
import time
from typing import Any

import numpy
import pywintypes

from .protocol import (
    PROTOCOL_VERSION,
    ProtocolError,
    close_server,
    connect_server,
    create_server,
    read_frame,
    write_frame,
)


def _as_python(value: Any) -> Any:
    """Convert numpy containers to JSON-compatible Python values."""

    return value.tolist() if hasattr(value, "tolist") else value


def _prediction_json(prediction: Any) -> dict[str, Any]:
    """Read the documented Result.json attribute without persisting images."""

    value = getattr(prediction, "json", None)
    value = value() if callable(value) else value
    if isinstance(value, str):
        import json

        value = json.loads(value)
    if not isinstance(value, dict):
        raise RuntimeError("PaddleOCR result did not expose a JSON object")
    result = value.get("res", value)
    if not isinstance(result, dict):
        raise RuntimeError("PaddleOCR result JSON has no res object")
    return {str(key): _as_python(item) for key, item in result.items()}


def _model_name(model: str) -> tuple[str, str]:
    """Resolve an ArgusFlow profile to the official PP-OCRv6 pair."""

    if model == "pp_ocr_v6_tiny":
        return "PP-OCRv6_tiny_det", "PP-OCRv6_tiny_rec"
    if model == "pp_ocr_v6_medium":
        return "PP-OCRv6_medium_det", "PP-OCRv6_medium_rec"
    raise ProtocolError("invalid_model", f"unsupported OCR model: {model}")


class PaddleModelPool:
    """Lazily owns one PaddleOCR pipeline per model tier."""

    def __init__(self) -> None:
        self.device = os.environ.get("ARGUSFLOW_PADDLE_DEVICE", "cpu")
        self._pipelines: dict[tuple[str, str, bool, bool, bool], Any] = {}
        self.current_model: str | None = None
        try:
            self.paddleocr_version = importlib.metadata.version("paddleocr")
        except importlib.metadata.PackageNotFoundError:
            self.paddleocr_version = "3.7.0"

    def pipeline(self, model: str, options: dict[str, Any]) -> Any:
        """Load a profile once, keeping document-only preprocessing disabled by default."""

        language = str(options.get("language", "ch"))
        orientation = bool(options.get("use_doc_orientation_classify", False))
        unwarping = bool(options.get("use_doc_unwarping", False))
        textline_orientation = bool(options.get("use_textline_orientation", False))
        key = (model, language, orientation, unwarping, textline_orientation)
        if key not in self._pipelines:
            from paddleocr import PaddleOCR

            detection_model, recognition_model = _model_name(model)
            self._pipelines[key] = PaddleOCR(
                text_detection_model_name=detection_model,
                text_recognition_model_name=recognition_model,
                lang=language,
                device=self.device,
                engine="paddle",
                use_doc_orientation_classify=orientation,
                use_doc_unwarping=unwarping,
                use_textline_orientation=textline_orientation,
            )
        self.current_model = model
        return self._pipelines[key]


class VisionWorker:
    """Worker state machine with latest-request backpressure at the pipe boundary."""

    def __init__(self) -> None:
        self.models = PaddleModelPool()
        self.lifecycle = "starting"
        self.queue_depth = 0
        self.worker_version = "argusflow-vision-worker/0.1.0"

    def health(self) -> dict[str, Any]:
        """Return the stable health schema consumed by WorkerHealth."""

        model = None
        if self.models.current_model is not None:
            model = {
                "model": self.models.current_model,
                "device": self.models.device,
                "engine": "paddle_static",
            }
        return {
            "protocol_version": PROTOCOL_VERSION,
            "worker_version": self.worker_version,
            "paddleocr_version": self.models.paddleocr_version,
            "lifecycle": self.lifecycle,
            "model": model,
            "queue_depth": self.queue_depth,
        }

    def prewarm(self) -> None:
        """Load and exercise both OCR tiers before accepting requests."""

        self.lifecycle = "loading_models"
        warmup = numpy.zeros((64, 256, 3), dtype=numpy.uint8)
        for model in ("pp_ocr_v6_tiny", "pp_ocr_v6_medium"):
            for language in ("ch", "en"):
                pipeline = self.models.pipeline(model, {"language": language})
                next(iter(pipeline.predict(warmup)), None)
        self.lifecycle = "ready"

    def recognize(self, request: dict[str, Any], body: bytes) -> dict[str, Any]:
        """Decode a Rust ROI binary body, run PaddleOCR, and return frame-local polygons."""

        started = time.perf_counter()
        transport = request.get("pixel_transport")
        if not isinstance(transport, dict) or transport.get("type") != "inline_bytes":
            raise ProtocolError(
                "unsupported_transport",
                "P0 worker accepts only inline_bytes; shared memory is reserved for P1",
            )
        width = int(transport["width"])
        height = int(transport["height"])
        stride = int(transport["stride_bytes"])
        if width <= 0 or height <= 0 or stride < width * 4:
            raise ProtocolError("invalid_pixels", "ROI dimensions or stride are invalid")
        required_bytes = stride * height
        declared_length = int(transport.get("body_length", -1))
        if declared_length != required_bytes or len(body) != required_bytes:
            raise ProtocolError("invalid_pixels", "binary pixel body length must equal stride*height")
        bgra = numpy.frombuffer(memoryview(body), dtype=numpy.uint8, count=required_bytes).reshape(
            height, stride
        )[:, : width * 4]
        rgb = bgra.reshape(height, width, 4)[:, :, :3][:, :, ::-1].copy()
        profile = request.get("profile")
        if not isinstance(profile, dict):
            raise ProtocolError("invalid_profile", "OCR profile is missing")
        model = str(profile.get("model"))
        options = profile.get("options")
        if not isinstance(options, dict):
            raise ProtocolError("invalid_profile", "OCR options are missing")
        pipeline = self.models.pipeline(model, options)
        prediction: Any = next(iter(pipeline.predict(rgb)), None)
        if prediction is None:
            items: list[dict[str, Any]] = []
        else:
            result = _prediction_json(prediction)
            items = _items_from_result(result, request.get("roi"))
        deadline_ms = int(request.get("deadline_ms", 0))
        elapsed_ms = int((time.perf_counter() - started) * 1000)
        if deadline_ms > 0 and elapsed_ms > deadline_ms:
            raise ProtocolError("deadline_exceeded", "OCR request exceeded its deadline")
        return {
            "request_id": str(request["request_id"]),
            "frame_id": int(request["frame_id"]),
            "topology_generation": int(request["topology_generation"]),
            "model": model,
            "elapsed_ms": elapsed_ms,
            "items": items,
        }


def _items_from_result(result: dict[str, Any], roi: Any) -> list[dict[str, Any]]:
    """Align rec_texts, rec_scores and rec_polys while preserving raw text."""

    if not isinstance(roi, dict):
        raise ProtocolError("invalid_roi", "OCR request ROI is missing")
    offset_x = int(roi["x"])
    offset_y = int(roi["y"])
    texts = list(result.get("rec_texts", []))
    scores = list(result.get("rec_scores", []))
    polygons = list(result.get("rec_polys", []))
    if not polygons:
        boxes = list(result.get("rec_boxes", []))
        polygons = [
            [[box[0], box[1]], [box[2], box[1]], [box[2], box[3]], [box[0], box[3]]]
            for box in boxes
            if len(box) >= 4
        ]
    items: list[dict[str, Any]] = []
    for index, text in enumerate(texts):
        polygon = polygons[index] if index < len(polygons) else []
        points = [
            [float(point[0]) + offset_x, float(point[1]) + offset_y]
            for point in polygon
            if len(point) >= 2
        ]
        if not points:
            continue
        score = float(scores[index]) if index < len(scores) else 0.0
        items.append(
            {
                "raw_text": str(text),
                "confidence": max(0.0, min(1.0, score)),
                "polygon": [{"x": point[0], "y": point[1]} for point in points],
            }
        )
    return items


def _response(envelope: dict[str, Any], payload: dict[str, Any]) -> dict[str, Any]:
    """Wrap a response with protocol and session correlation fields."""

    return {
        "protocol_version": PROTOCOL_VERSION,
        "request_id": str(envelope.get("request_id", "")),
        "session_token": str(envelope.get("session_token", "")),
        "payload": payload,
    }


def _error_response(envelope: dict[str, Any], code: str, message: str) -> dict[str, Any]:
    """Return a structured worker error without returning pixel data."""

    return _response(
        envelope,
        {
            "kind": "recognize",
            "response": None,
            "error": {"code": code, "message": message},
        },
    )


def serve(pipe_name: str, session_token: str) -> None:
    """Supervise worker state and recreate the model pool after a bad inference."""

    while True:
        worker = VisionWorker()
        try:
            worker.prewarm()
        except Exception as error:
            worker.lifecycle = "failed"
            startup_error = str(error)
        else:
            startup_error = None

        handle = create_server(pipe_name)
        restart_worker = False
        try:
            connect_server(handle)
            while True:
                try:
                    envelope, body = read_frame(handle)
                    if envelope.get("protocol_version") != PROTOCOL_VERSION:
                        write_frame(
                            handle,
                            _error_response(
                                envelope,
                                "protocol_mismatch",
                                "worker protocol version mismatch",
                            ),
                        )
                        continue
                    if envelope.get("session_token") != session_token:
                        write_frame(
                            handle,
                            _error_response(envelope, "unauthorized", "session token mismatch"),
                        )
                        continue
                    payload = envelope.get("payload")
                    if not isinstance(payload, dict):
                        raise ProtocolError("invalid_message", "payload must be an object")
                    kind = payload.get("kind")
                    if kind == "health":
                        if body:
                            raise ProtocolError(
                                "unexpected_body",
                                "health requests must not carry a binary body",
                            )
                        health = worker.health()
                        if startup_error is not None:
                            health["lifecycle"] = "failed"
                        write_frame(handle, _response(envelope, {"kind": "health", "health": health}))
                    elif kind == "recognize":
                        if startup_error is not None:
                            write_frame(
                                handle,
                                _error_response(envelope, "worker_failed", startup_error),
                            )
                            continue
                        request = payload.get("request")
                        if not isinstance(request, dict):
                            raise ProtocolError("invalid_request", "recognize request is missing")
                        worker.queue_depth = 1
                        try:
                            result = worker.recognize(request, body)
                        except ProtocolError as error:
                            write_frame(handle, _error_response(envelope, error.code, error.message))
                            if error.code == "deadline_exceeded":
                                # Paddle predict is synchronous and cannot be cooperatively
                                # cancelled; rebuild the model pool before accepting a new
                                # connection so the Rust client can perform a fresh handshake.
                                worker.lifecycle = "failed"
                                restart_worker = True
                                break
                        except Exception as error:
                            write_frame(handle, _error_response(envelope, "ocr_failed", str(error)))
                        else:
                            write_frame(
                                handle,
                                _response(
                                    envelope,
                                    {
                                        "kind": "recognize",
                                        "response": result,
                                        "error": None,
                                    },
                                ),
                            )
                        finally:
                            worker.queue_depth = 0
                    else:
                        raise ProtocolError("unknown_command", f"unknown worker command: {kind}")
                except ProtocolError as error:
                    try:
                        write_frame(
                            handle,
                            _error_response({}, error.code, error.message),
                        )
                    except (OSError, pywintypes.error):
                        restart_worker = True
                        break
                except (OSError, pywintypes.error):
                    # The Rust client drops the pipe on its deadline. Recreate the model
                    # pool and wait for a new authenticated connection.
                    worker.lifecycle = "failed"
                    restart_worker = True
                    break
        finally:
            close_server(handle)
        if restart_worker:
            time.sleep(0.05)
