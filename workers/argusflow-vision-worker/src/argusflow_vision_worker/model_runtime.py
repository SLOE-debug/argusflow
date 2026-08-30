"""OCR model ownership, tier readiness, and background prewarming."""

from __future__ import annotations

import importlib.metadata
import os
import threading
from collections.abc import Callable
from typing import Any

from .device import DeviceKind, InferenceDevice, select_inference_device
from .protocol import PROTOCOL_VERSION, ProtocolError

SMALL_MODEL = "pp_ocr_v6_small"
MEDIUM_MODEL = "pp_ocr_v6_medium"
MODEL_TIERS = (SMALL_MODEL, MEDIUM_MODEL)
PipelineKey = tuple[str, bool, bool, bool]


def model_names(model: str) -> tuple[str, str]:
    """Resolve an ArgusFlow model enum to the official PP-OCRv6 pair."""

    if model == SMALL_MODEL:
        return "PP-OCRv6_small_det", "PP-OCRv6_small_rec"
    if model == MEDIUM_MODEL:
        return "PP-OCRv6_medium_det", "PP-OCRv6_medium_rec"
    raise ProtocolError("invalid_model", f"unsupported OCR model: {model}")


def minimum_score(model: str) -> float:
    """Return the tier-specific junk-text filter shared by inference and adaptation."""

    if model == SMALL_MODEL:
        return 0.35
    if model == MEDIUM_MODEL:
        return 0.30
    raise ProtocolError("invalid_model", f"unsupported OCR model: {model}")


class PaddleModelPool:
    """Own one cached PaddleOCR pipeline for each model/options combination."""

    def __init__(self, device: InferenceDevice) -> None:
        self.device = device
        self._pipelines: dict[PipelineKey, Any] = {}
        self._cache_lock = threading.Lock()
        self._construction_locks: dict[PipelineKey, threading.Lock] = {}
        try:
            self.paddleocr_version = importlib.metadata.version("paddleocr")
        except importlib.metadata.PackageNotFoundError:
            self.paddleocr_version = "unknown"

    def pipeline(self, model: str, options: dict[str, Any]) -> Any:
        """Load an exact model configuration once and reuse its predictor."""

        orientation = bool(options.get("use_doc_orientation_classify", False))
        unwarping = bool(options.get("use_doc_unwarping", False))
        textline_orientation = bool(options.get("use_textline_orientation", False))
        key: PipelineKey = (model, orientation, unwarping, textline_orientation)
        with self._cache_lock:
            cached = self._pipelines.get(key)
            construction_lock = self._construction_locks.setdefault(key, threading.Lock())
        if cached is not None:
            return cached

        # 每个精确配置只构造一次，但不同模型不共享锁，因此 Small 与 Medium 可真正并行加载。
        with construction_lock:
            with self._cache_lock:
                cached = self._pipelines.get(key)
            if cached is not None:
                return cached

            from paddleocr import PaddleOCR

            detection_model, recognition_model = model_names(model)
            pipeline = PaddleOCR(
                text_detection_model_name=detection_model,
                text_recognition_model_name=recognition_model,
                device=self.device.paddle_name,
                engine="paddle",
                use_doc_orientation_classify=orientation,
                use_doc_unwarping=unwarping,
                use_textline_orientation=textline_orientation,
                text_recognition_batch_size=32 if self.device.kind is DeviceKind.CUDA else 8,
                enable_mkldnn=self.device.kind is DeviceKind.CPU,
                cpu_threads=max(1, min(8, os.cpu_count() or 4)),
            )
            with self._cache_lock:
                self._pipelines[key] = pipeline
            return pipeline


class OcrModelRuntime:
    """Select one inference device, then load and warm both OCR tiers in parallel."""

    def __init__(self, status_changed: Callable[[dict[str, Any]], None] | None = None) -> None:
        self._lock = threading.RLock()
        self._publish_lock = threading.Lock()
        self._initialization: threading.Thread | None = None
        self._status_changed = status_changed
        self._pool: PaddleModelPool | None = None
        self._lifecycle = "starting"
        self._degradation_reason: str | None = None
        self._models: dict[str, dict[str, Any]] = {}

    def start(self) -> None:
        """Start initialization once and return before Paddle imports or model downloads finish."""

        with self._lock:
            if self._initialization is not None and self._initialization.is_alive():
                return
            self._lifecycle = "selecting_device"
            self._models = {}
            self._degradation_reason = None
            self._initialization = threading.Thread(
                target=self._initialize,
                name="argusflow-ocr-initializer",
                daemon=True,
            )
            self._initialization.start()
        self._publish()

    def health(self, queue_depth: int) -> dict[str, Any]:
        """Return a consistent protocol snapshot without waiting for model work."""

        with self._lock:
            pool = self._pool
            return {
                "protocol_version": PROTOCOL_VERSION,
                "worker_version": "argusflow-vision-worker/0.1.0",
                "paddleocr_version": pool.paddleocr_version if pool else "",
                "lifecycle": self._lifecycle,
                "models": [dict(self._models[model]) for model in MODEL_TIERS if model in self._models],
                "queue_depth": queue_depth,
                "degradation_reason": self._degradation_reason,
            }

    def pipeline(self, model: str, options: dict[str, Any]) -> Any:
        """Return a ready tier or fail immediately with its current lifecycle."""

        with self._lock:
            state = self._models.get(model)
            pool = self._pool
            if state is None or state["lifecycle"] != "ready" or pool is None:
                lifecycle = state["lifecycle"] if state else "pending"
                raise ProtocolError("model_not_ready", f"OCR model {model} is {lifecycle}")
        return pool.pipeline(model, options)

    def _initialize(self) -> None:
        """Select the device and coordinate parallel model warmups off the pipe thread."""

        try:
            selection = select_inference_device()
            self._install_pool(selection.device, selection.degradation_reason)
            failures = self._warm_tiers_in_parallel()
            if failures and selection.device.kind is DeviceKind.CUDA:
                # Predictor construction can fail after the tensor probe. Wait for both CUDA jobs,
                # then rebuild the complete model set on CPU so requests never mix devices.
                cpu = InferenceDevice(DeviceKind.CPU)
                import paddle

                paddle.set_device("cpu")
                first_failure = next(failures[model] for model in MODEL_TIERS if model in failures)
                self._install_pool(
                    cpu,
                    f"GPU model initialization failed; switched to CPU: {first_failure}",
                )
                failures = self._warm_tiers_in_parallel()
            self._finish_initialization(failures)
        except Exception as error:
            with self._lock:
                self._lifecycle = "failed"
                if not self._models:
                    self._degradation_reason = str(error)
            self._publish()

    def _warm_tiers_in_parallel(self) -> dict[str, Exception]:
        """Run both blocking Paddle constructors and warmups concurrently, then join them."""

        # 每个任务只写入自己的模型状态；共享失败集合由独立锁保护。
        failures: dict[str, Exception] = {}
        failures_lock = threading.Lock()

        def warm(model: str) -> None:
            try:
                self._warm_tier(model)
            except Exception as error:
                with failures_lock:
                    failures[model] = error

        threads = [
            threading.Thread(
                target=warm,
                args=(model,),
                name=f"argusflow-ocr-{model}-warmer",
                daemon=True,
            )
            for model in MODEL_TIERS
        ]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        return failures

    def _finish_initialization(self, failures: dict[str, Exception]) -> None:
        """Publish final model failures or mark the complete parallel batch available."""

        for model in MODEL_TIERS:
            if model in failures:
                self._set_model_state(model, "failed", str(failures[model]))

        small_failure = failures.get(SMALL_MODEL)
        medium_failure = failures.get(MEDIUM_MODEL)
        with self._lock:
            if small_failure is not None:
                self._lifecycle = "failed"
                self._degradation_reason = f"Small OCR model is unavailable: {small_failure}"
            elif medium_failure is not None:
                self._lifecycle = "degraded"
                medium_reason = f"Medium OCR model is unavailable: {medium_failure}"
                self._degradation_reason = (
                    f"{self._degradation_reason}; {medium_reason}"
                    if self._degradation_reason
                    else medium_reason
                )
            else:
                self._lifecycle = "degraded" if self._degradation_reason else "ready"
        self._publish()

    def _install_pool(self, device: InferenceDevice, degradation_reason: str | None) -> None:
        """Replace the complete pool and reset both tier state records."""

        pool = PaddleModelPool(device)
        with self._lock:
            self._pool = pool
            self._degradation_reason = degradation_reason
            self._lifecycle = "loading_models"
            self._models = {
                model: {
                    "model": model,
                    "device": device.as_wire(),
                    "engine": "paddle_static",
                    "lifecycle": "pending",
                    "message": None,
                }
                for model in MODEL_TIERS
            }
        self._publish()

    def _warm_tier(self, model: str) -> None:
        """Load one tier and execute detection plus recognition on synthetic text."""

        import cv2
        import numpy

        self._set_model_state(model, "loading")
        with self._lock:
            pool = self._pool
        if pool is None:
            raise RuntimeError("OCR model pool was not installed")
        pipeline = pool.pipeline(model, {})
        self._set_model_state(model, "warming")
        warmup = numpy.full((128, 640, 3), 255, dtype=numpy.uint8)
        cv2.putText(
            warmup,
            "ARGUSFLOW OCR 123",
            (18, 82),
            cv2.FONT_HERSHEY_SIMPLEX,
            1.5,
            (0, 0, 0),
            3,
            cv2.LINE_AA,
        )
        next(iter(pipeline.predict(warmup, text_rec_score_thresh=minimum_score(model))), None)
        self._set_model_state(model, "ready")

    def _set_model_state(self, model: str, lifecycle: str, message: str | None = None) -> None:
        """Update one tier without exposing a partially mutated health record."""

        with self._lock:
            self._models[model]["lifecycle"] = lifecycle
            self._models[model]["message"] = message
        self._publish()

    def _publish(self) -> None:
        """Notify the deployment status-file adapter after a lifecycle transition."""

        if self._status_changed is not None:
            # 并行模型任务可能同时切换状态；发布锁保证原子文件适配器不会复用同一临时文件。
            with self._publish_lock:
                self._status_changed(self.health(0))
