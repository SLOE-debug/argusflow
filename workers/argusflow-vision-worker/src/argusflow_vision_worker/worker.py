"""PaddleOCR 3.7.0 adapter and bounded Named Pipe request loop."""

from __future__ import annotations

import importlib.metadata
import json
import os
import time
from pathlib import Path
from typing import Any

import numpy
import pywintypes

from .image_preprocessing import ImagePreprocessingMode, PreparedOcrImage, prepare_ocr_image
from .diagnostic_artifact import encode_exact_model_input
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
    if model == "pp_ocr_v6_small":
        return "PP-OCRv6_small_det", "PP-OCRv6_small_rec"
    if model == "pp_ocr_v6_medium":
        return "PP-OCRv6_medium_det", "PP-OCRv6_medium_rec"
    raise ProtocolError("invalid_model", f"unsupported OCR model: {model}")


class PaddleModelPool:
    """Lazily owns one PaddleOCR pipeline per model tier."""

    def __init__(self) -> None:
        self.device = os.environ.get("ARGUSFLOW_PADDLE_DEVICE", "cpu")
        self._pipelines: dict[tuple[str, bool, bool, bool], Any] = {}
        self.current_model: str | None = None
        try:
            self.paddleocr_version = importlib.metadata.version("paddleocr")
        except importlib.metadata.PackageNotFoundError:
            self.paddleocr_version = "3.7.0"

    def pipeline(self, model: str, options: dict[str, Any]) -> Any:
        """Load a profile once, keeping document-only preprocessing disabled by default."""

        orientation = bool(options.get("use_doc_orientation_classify", False))
        unwarping = bool(options.get("use_doc_unwarping", False))
        textline_orientation = bool(options.get("use_textline_orientation", False))
        # Explicit PP-OCRv6 model names define the recognition dictionary; PaddleOCR ignores
        # `lang` in this mode, so it must not create duplicate pipelines for the same models.
        key = (model, orientation, unwarping, textline_orientation)
        if key not in self._pipelines:
            from paddleocr import PaddleOCR

            detection_model, recognition_model = _model_name(model)
            self._pipelines[key] = PaddleOCR(
                text_detection_model_name=detection_model,
                text_recognition_model_name=recognition_model,
                device=self.device,
                engine="paddle",
                use_doc_orientation_classify=orientation,
                use_doc_unwarping=unwarping,
                use_textline_orientation=textline_orientation,
                text_recognition_batch_size=8,
                enable_mkldnn=self.device == "cpu",
                cpu_threads=max(1, min(8, os.cpu_count() or 4)),
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
        """Load the default desktop tier; exceptional Tiny/Medium tiers stay lazy."""

        self.lifecycle = "loading_models"
        warmup = numpy.zeros((64, 256, 3), dtype=numpy.uint8)
        warmup_options = {"image_preprocessing": "none"}
        model = "pp_ocr_v6_small"
        pipeline = self.models.pipeline(model, warmup_options)
        next(iter(pipeline.predict(warmup, text_rec_score_thresh=_minimum_score(model))), None)
        self.lifecycle = "ready"

    def recognize(
        self,
        request: dict[str, Any],
        body: bytes,
    ) -> tuple[dict[str, Any], dict[str, str | int] | None, bytes]:
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
        try:
            preprocessing_mode = ImagePreprocessingMode(str(options["image_preprocessing"]))
        except (KeyError, ValueError) as error:
            raise ProtocolError(
                "invalid_profile",
                "OCR image_preprocessing is missing or unsupported",
            ) from error
        prepared = prepare_ocr_image(rgb, preprocessing_mode)
        preprocess_elapsed_ms = int((time.perf_counter() - started) * 1000)
        pipeline = self.models.pipeline(model, options)
        minimum_score = _minimum_score(model)
        inference_started = time.perf_counter()
        prediction: Any = next(
            iter(
                pipeline.predict(
                    prepared.pixels,
                    text_rec_score_thresh=minimum_score,
                )
            ),
            None,
        )
        inference_elapsed_ms = int((time.perf_counter() - inference_started) * 1000)
        if prediction is None:
            items: list[dict[str, Any]] = []
        else:
            result = _prediction_json(prediction)
            items = _items_from_result(
                result,
                request.get("roi"),
                prepared,
                minimum_score,
            )
        deadline_ms = int(request.get("deadline_ms", 0))
        elapsed_ms = int((time.perf_counter() - started) * 1000)
        if deadline_ms > 0 and elapsed_ms > deadline_ms:
            raise ProtocolError("deadline_exceeded", "OCR request exceeded its deadline")
        diagnostics = request.get("diagnostics")
        if not isinstance(diagnostics, dict):
            raise ProtocolError("invalid_diagnostics", "OCR diagnostics options are missing")
        capture_model_input = bool(diagnostics.get("capture_model_input", False))
        encoding = str(diagnostics.get("encoding", ""))
        if capture_model_input and encoding != "png":
            raise ProtocolError("invalid_diagnostics", "model-input diagnostics require PNG")
        artifact = encode_exact_model_input(prepared.pixels) if capture_model_input else None
        response = {
            "request_id": str(request["request_id"]),
            "frame_id": int(request["frame_id"]),
            "topology_generation": int(request["topology_generation"]),
            "model": model,
            "elapsed_ms": elapsed_ms,
            "preprocessing": prepared.summary(),
            "timings": {
                "preprocess_elapsed_ms": preprocess_elapsed_ms,
                "inference_elapsed_ms": inference_elapsed_ms,
            },
            "items": items,
        }
        return response, artifact.metadata() if artifact else None, artifact.body if artifact else b""


def _minimum_score(model: str) -> float:
    """Return the tier-specific junk-text filter used by Paddle and the response adapter."""

    if model == "pp_ocr_v6_tiny":
        return 0.45
    if model == "pp_ocr_v6_small":
        return 0.35
    if model == "pp_ocr_v6_medium":
        return 0.30
    raise ProtocolError("invalid_model", f"unsupported OCR model: {model}")


def _items_from_result(
    result: dict[str, Any],
    roi: Any,
    prepared: PreparedOcrImage,
    minimum_score: float,
) -> list[dict[str, Any]]:
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
            [
                prepared.map_x_to_input(float(point[0])) + offset_x,
                prepared.map_y_to_input(float(point[1])) + offset_y,
            ]
            for point in polygon
            if len(point) >= 2
        ]
        if not points:
            continue
        score = float(scores[index]) if index < len(scores) else 0.0
        if score < minimum_score:
            continue
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
            "artifact": None,
            "error": {"code": code, "message": message},
        },
    )


def _publish_startup_status(
    status_file: str | None,
    lifecycle: str,
    message: str | None = None,
) -> None:
    """Atomically publish model readiness without exposing captured content."""

    if status_file is None:
        return
    status_path = Path(status_file)
    status_path.parent.mkdir(parents=True, exist_ok=True)
    status = {"lifecycle": lifecycle}
    if message is not None:
        status["message"] = message
    temporary_path = status_path.with_name(f"{status_path.name}.{os.getpid()}.tmp")
    temporary_path.write_text(json.dumps(status), encoding="utf-8")
    os.replace(temporary_path, status_path)


def serve(pipe_name: str, session_token: str, status_file: str | None = None) -> None:
    """Supervise worker state and recreate the model pool after a bad inference."""

    while True:
        worker = VisionWorker()
        _publish_startup_status(status_file, "loading_models")
        try:
            worker.prewarm()
        except Exception as error:
            worker.lifecycle = "failed"
            startup_error = str(error)
            _publish_startup_status(status_file, "failed", startup_error)
        else:
            startup_error = None

        handle = create_server(pipe_name)
        if startup_error is None:
            # Named Pipe 已经存在后再发布 ready，避免桌面端首次 health handshake 抢跑。
            _publish_startup_status(status_file, "ready")
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
                            result, artifact, artifact_body = worker.recognize(request, body)
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
                                        "artifact": artifact,
                                        "error": None,
                                    },
                                ),
                                artifact_body,
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
