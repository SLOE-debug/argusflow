"""Model-pool tests that do not load Paddle native models."""

from __future__ import annotations

import sys
import types
import unittest
from unittest.mock import patch

from argusflow_vision_worker.worker import PaddleModelPool, VisionWorker


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

    def test_explicit_model_pair_does_not_duplicate_pipeline_by_language(self) -> None:
        pool = PaddleModelPool()

        chinese = pool.pipeline("pp_ocr_v6_small", {"language": "ch"})
        english = pool.pipeline("pp_ocr_v6_small", {"language": "en"})

        self.assertIs(chinese, english)
        self.assertEqual(len(FakePaddleOcr.created), 1)
        self.assertNotIn("lang", FakePaddleOcr.created[0].options)

    def test_prewarm_only_loads_the_default_desktop_tier(self) -> None:
        worker = VisionWorker()

        worker.prewarm()

        self.assertEqual(worker.lifecycle, "ready")
        self.assertEqual(len(FakePaddleOcr.created), 1)
        model_pairs = {
            (
                pipeline.options["text_detection_model_name"],
                pipeline.options["text_recognition_model_name"],
            )
            for pipeline in FakePaddleOcr.created
        }
        self.assertEqual(
            model_pairs,
            {("PP-OCRv6_small_det", "PP-OCRv6_small_rec")},
        )


if __name__ == "__main__":
    unittest.main()
