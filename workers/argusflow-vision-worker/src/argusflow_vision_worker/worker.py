"""Responsive Named Pipe server and PaddleOCR request orchestration."""

from __future__ import annotations

import json
import mmap
import os
import threading
import time
from pathlib import Path
from typing import Any

import pywintypes

from .model_runtime import OcrModelRuntime, minimum_score
from .protocol import (
    PROTOCOL_VERSION,
    ProtocolError,
    close_server,
    connect_server,
    create_server,
    read_frame,
    write_frame,
)


class VisionWorker:
    """Coordinate request backpressure with a separately initialized model runtime."""

    def __init__(self, status_file: str | None = None) -> None:
        self._queue_lock = threading.Lock()
        self._queue_depth = 0
        self.models = OcrModelRuntime(
            lambda health: _publish_startup_status(status_file, health)
        )

    def initialize(self) -> None:
        """Begin device selection and model prewarming after the frontend has painted."""

        self.models.start()

    def health(self) -> dict[str, Any]:
        """Return the stable health schema consumed by Rust WorkerHealth."""

        with self._queue_lock:
            queue_depth = self._queue_depth
        return self.models.health(queue_depth)

    def recognize(
        self,
        request: dict[str, Any],
        body: bytes,
    ) -> tuple[dict[str, Any], dict[str, str | int] | None, bytes]:
        """Map a Rust ROI, run a ready PaddleOCR tier, and return frame-local polygons."""

        with self._queue_lock:
            self._queue_depth = 1
        try:
            return self._recognize(request, body)
        finally:
            with self._queue_lock:
                self._queue_depth = 0

    def _recognize(
        self,
        request: dict[str, Any],
        body: bytes,
    ) -> tuple[dict[str, Any], dict[str, str | int] | None, bytes]:
        """Execute one request after the queue-depth guard has been installed."""

        from .diagnostic_artifact import encode_exact_model_input
        from .image_preprocessing import ImagePreprocessingMode, prepare_ocr_image
        from .result_adapter import items_from_prediction

        started = time.perf_counter()
        if body:
            raise ProtocolError("unexpected_body", "OCR pixels must use shared memory")
        rgb = _read_shared_pixels(request.get("pixels"))
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
        score_threshold = minimum_score(model)
        inference_started = time.perf_counter()
        prediction: Any = next(
            iter(
                pipeline.predict(
                    prepared.pixels,
                    text_rec_score_thresh=score_threshold,
                )
            ),
            None,
        )
        inference_elapsed_ms = int((time.perf_counter() - inference_started) * 1000)
        items = (
            []
            if prediction is None
            else items_from_prediction(
                prediction,
                request.get("roi"),
                prepared,
                score_threshold,
            )
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


def _read_shared_pixels(transport: Any) -> Any:
    """Copy one BGRA mapping into a compact RGB array before releasing its lease."""

    import numpy

    if not isinstance(transport, dict):
        raise ProtocolError("invalid_pixels", "shared-memory pixel metadata is missing")
    mapping_name = str(transport.get("mapping_name", ""))
    lease_id = str(transport.get("lease_id", ""))
    width = int(transport.get("width", 0))
    height = int(transport.get("height", 0))
    stride = int(transport.get("stride_bytes", 0))
    length = int(transport.get("length", 0))
    if not mapping_name or not lease_id:
        raise ProtocolError("invalid_pixels", "shared-memory mapping or lease ID is empty")
    if width <= 0 or height <= 0 or stride < width * 4 or length != stride * height:
        raise ProtocolError(
            "invalid_pixels",
            "shared-memory dimensions, stride, or length are invalid",
        )
    try:
        mapping = mmap.mmap(-1, length, tagname=mapping_name, access=mmap.ACCESS_READ)
    except (OSError, ValueError) as error:
        raise ProtocolError("shared_memory_unavailable", str(error)) from error
    try:
        bgra = numpy.frombuffer(mapping, dtype=numpy.uint8, count=length).reshape(height, stride)
        rgb = bgra[:, : width * 4].reshape(height, width, 4)[:, :, :3][:, :, ::-1].copy()
        del bgra
        return rgb
    finally:
        mapping.close()


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


def _publish_startup_status(status_file: str | None, health: dict[str, Any]) -> None:
    """Atomically publish model readiness without exposing captured content."""

    if status_file is None:
        return
    status_path = Path(status_file)
    status_path.parent.mkdir(parents=True, exist_ok=True)
    temporary_path = status_path.with_name(f"{status_path.name}.{os.getpid()}.tmp")
    temporary_path.write_text(json.dumps(health), encoding="utf-8")
    os.replace(temporary_path, status_path)


def _validate_envelope(
    envelope: dict[str, Any],
    body: bytes,
    session_token: str,
) -> dict[str, Any]:
    """Validate the common authenticated envelope and return its command payload."""

    if envelope.get("protocol_version") != PROTOCOL_VERSION:
        raise ProtocolError("protocol_mismatch", "worker protocol version mismatch")
    if envelope.get("session_token") != session_token:
        raise ProtocolError("unauthorized", "session token mismatch")
    payload = envelope.get("payload")
    if not isinstance(payload, dict):
        raise ProtocolError("invalid_message", "payload must be an object")
    if payload.get("kind") in {"health", "initialize"} and body:
        raise ProtocolError("unexpected_body", "lifecycle requests must not carry a binary body")
    return payload


def _serve_connection(
    handle: pywintypes.HANDLE,
    worker: VisionWorker,
    session_token: str,
) -> None:
    """Serve one authenticated client until it disconnects, preserving loaded models."""

    while True:
        envelope, body = read_frame(handle)
        try:
            payload = _validate_envelope(envelope, body, session_token)
            kind = payload.get("kind")
            if kind == "health":
                write_frame(
                    handle,
                    _response(envelope, {"kind": "health", "health": worker.health()}),
                )
            elif kind == "initialize":
                worker.initialize()
                write_frame(
                    handle,
                    _response(envelope, {"kind": "health", "health": worker.health()}),
                )
            elif kind == "recognize":
                request = payload.get("request")
                if not isinstance(request, dict):
                    raise ProtocolError("invalid_request", "recognize request is missing")
                result, artifact, artifact_body = worker.recognize(request, body)
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
            else:
                raise ProtocolError("unknown_command", f"unknown worker command: {kind}")
        except ProtocolError as error:
            write_frame(handle, _error_response(envelope, error.code, error.message))
        except Exception as error:
            write_frame(handle, _error_response(envelope, "ocr_failed", str(error)))


def serve(pipe_name: str, session_token: str, status_file: str | None = None) -> None:
    """Expose an idle worker immediately and initialize models only on explicit request."""

    worker = VisionWorker(status_file)
    while True:
        handle = create_server(pipe_name)
        try:
            connect_server(handle)
            _serve_connection(handle, worker, session_token)
        except (OSError, pywintypes.error):
            # A Rust deadline intentionally drops the connection. Only the transport is recreated;
            # immutable model weights and warmed predictors remain owned by the process.
            pass
        finally:
            close_server(handle)
