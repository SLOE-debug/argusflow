"""Model-pool tests that do not load Paddle native models."""

from __future__ import annotations

import sys
import threading
import time
import types
import unittest
from unittest.mock import patch

from argusflow_vision_worker.device import DeviceKind, DeviceSelection, InferenceDevice
from argusflow_vision_worker.model_runtime import OcrModelRuntime, PaddleModelPool


def _medium_is_ready(health: dict[str, object]) -> bool:
    """Return whether the final declared model tier completed its warmup."""

    models = health["models"]
    return isinstance(models, list) and bool(models) and models[-1]["lifecycle"] == "ready"


class FakePaddleOcr:
    """Record constructor arguments and return an empty deterministic prediction."""

    created: list[FakePaddleOcr] = []
    construction_barrier: threading.Barrier | None = None

    def __init__(self, **options: object) -> None:
        if self.construction_barrier is not None:
            self.construction_barrier.wait(timeout=0.5)
        self.options = options
        self.created.append(self)

    def predict(self, _image: object, **_options: object) -> list[object]:
        return []


class PaddleModelPoolTests(unittest.TestCase):
    """Verify ArgusFlow profile identity independently of Paddle imports."""

    def setUp(self) -> None:
        FakePaddleOcr.created.clear()
        FakePaddleOcr.construction_barrier = None
        paddleocr = types.ModuleType("paddleocr")
        paddleocr.PaddleOCR = FakePaddleOcr  # type: ignore[attr-defined]
        self.paddleocr_patch = patch.dict(sys.modules, {"paddleocr": paddleocr})
        self.paddleocr_patch.start()

    def tearDown(self) -> None:
        self.paddleocr_patch.stop()

    def test_runtime_stays_idle_until_frontend_requests_initialization(self) -> None:
        runtime = OcrModelRuntime()

        health = runtime.health(0)

        self.assertEqual(health["lifecycle"], "starting")
        self.assertEqual(health["models"], [])
        self.assertEqual(FakePaddleOcr.created, [])

    def test_explicit_model_pair_does_not_duplicate_pipeline_by_language(self) -> None:
        pool = PaddleModelPool(InferenceDevice(DeviceKind.CPU))

        chinese = pool.pipeline("pp_ocr_v6_small", {"language": "ch"})
        english = pool.pipeline("pp_ocr_v6_small", {"language": "en"})

        self.assertIs(chinese, english)
        self.assertEqual(len(FakePaddleOcr.created), 1)
        self.assertNotIn("lang", FakePaddleOcr.created[0].options)

    def test_distinct_model_constructors_do_not_share_a_serial_lock(self) -> None:
        pool = PaddleModelPool(InferenceDevice(DeviceKind.CPU))
        FakePaddleOcr.construction_barrier = threading.Barrier(2)
        failures: list[Exception] = []
        failures_lock = threading.Lock()

        def load(model: str) -> None:
            try:
                pool.pipeline(model, {})
            except Exception as error:
                with failures_lock:
                    failures.append(error)

        threads = [
            threading.Thread(target=load, args=(model,))
            for model in ("pp_ocr_v6_small", "pp_ocr_v6_medium")
        ]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join(timeout=1)

        self.assertFalse(any(thread.is_alive() for thread in threads))
        self.assertEqual(failures, [])
        self.assertEqual(len(FakePaddleOcr.created), 2)

    def test_both_model_tiers_are_warmed_in_parallel(self) -> None:
        runtime = OcrModelRuntime()
        selection = DeviceSelection(InferenceDevice(DeviceKind.CPU), None)
        both_started = threading.Event()
        release_warmups = threading.Event()
        started_models: list[str] = []
        started_models_lock = threading.Lock()

        def blocking_warmup(model: str) -> None:
            with started_models_lock:
                started_models.append(model)
                if len(started_models) == 2:
                    both_started.set()
            if not release_warmups.wait(timeout=1):
                raise TimeoutError("parallel model warmups were not released")
            runtime._set_model_state(model, "ready")  # noqa: SLF001 - verifies orchestration

        with patch(
            "argusflow_vision_worker.model_runtime.select_inference_device",
            return_value=selection,
        ), patch.object(runtime, "_warm_tier", side_effect=blocking_warmup):
            runtime.start()
            try:
                self.assertTrue(both_started.wait(timeout=0.5))
                self.assertCountEqual(
                    started_models,
                    ["pp_ocr_v6_small", "pp_ocr_v6_medium"],
                )
            finally:
                release_warmups.set()

            deadline = time.monotonic() + 1
            while not _medium_is_ready(runtime.health(0)):
                self.assertLess(time.monotonic(), deadline)
                time.sleep(0.01)

        self.assertEqual(runtime.health(0)["lifecycle"], "ready")

    def test_parallel_warmup_builds_both_explicit_model_pairs(self) -> None:
        runtime = OcrModelRuntime()
        selection = DeviceSelection(InferenceDevice(DeviceKind.CPU), None)

        with patch(
            "argusflow_vision_worker.model_runtime.select_inference_device",
            return_value=selection,
        ):
            runtime.start()
            deadline = time.monotonic() + 1
            while not _medium_is_ready(runtime.health(0)):
                self.assertLess(time.monotonic(), deadline)
                time.sleep(0.01)

        self.assertEqual(len(FakePaddleOcr.created), 2)
        model_pairs = {
            (
                pipeline.options["text_detection_model_name"],
                pipeline.options["text_recognition_model_name"],
            )
            for pipeline in FakePaddleOcr.created
        }
        self.assertEqual(
            model_pairs,
            {
                ("PP-OCRv6_small_det", "PP-OCRv6_small_rec"),
                ("PP-OCRv6_medium_det", "PP-OCRv6_medium_rec"),
            },
        )

    def test_cuda_uses_a_larger_recognition_batch(self) -> None:
        pool = PaddleModelPool(InferenceDevice(DeviceKind.CUDA, 0))

        pool.pipeline("pp_ocr_v6_small", {})

        self.assertEqual(FakePaddleOcr.created[0].options["text_recognition_batch_size"], 32)
        self.assertFalse(FakePaddleOcr.created[0].options["enable_mkldnn"])


if __name__ == "__main__":
    unittest.main()
