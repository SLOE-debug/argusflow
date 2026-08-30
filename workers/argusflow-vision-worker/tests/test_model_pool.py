"""Model-pool tests that do not load Paddle native models."""

from __future__ import annotations

import sys
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

    def __init__(self, **options: object) -> None:
        self.options = options
        self.created.append(self)

    def predict(self, _image: object, **_options: object) -> list[object]:
        return []


class PaddleModelPoolTests(unittest.TestCase):
    """Verify ArgusFlow profile identity independently of Paddle imports."""

    def setUp(self) -> None:
        FakePaddleOcr.created.clear()
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

    def test_small_becomes_ready_and_medium_is_warmed_in_the_background(self) -> None:
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

        self.assertEqual(runtime.health(0)["lifecycle"], "ready")
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
