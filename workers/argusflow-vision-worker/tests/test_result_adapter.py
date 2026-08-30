"""Direct Paddle Result mapping tests without JSON serialization."""

from __future__ import annotations

import unittest

import numpy

from argusflow_vision_worker.image_preprocessing import PreparedOcrImage
from argusflow_vision_worker.result_adapter import items_from_prediction


class ResultAdapterTests(unittest.TestCase):
    """Verify direct mapping fields and inverse ROI geometry."""

    def test_mapping_result_preserves_text_score_and_frame_offset(self) -> None:
        prepared = PreparedOcrImage(
            pixels=numpy.zeros((100, 200, 3), dtype=numpy.uint8),
            input_width=100,
            input_height=50,
            output_width=200,
            output_height=100,
            contrast_enhanced=False,
            sharpened=False,
            binarized=False,
        )
        prediction = {
            "rec_texts": ["ArgusFlow"],
            "rec_scores": numpy.asarray([0.98]),
            "rec_polys": [numpy.asarray([[20, 10], [120, 10], [120, 40], [20, 40]])],
        }

        items = items_from_prediction(
            prediction,
            {"x": 300, "y": 200},
            prepared,
            0.35,
        )

        self.assertEqual(items[0]["raw_text"], "ArgusFlow")
        self.assertAlmostEqual(items[0]["confidence"], 0.98, places=5)
        self.assertEqual(items[0]["polygon"][0], {"x": 310.0, "y": 205.0})


if __name__ == "__main__":
    unittest.main()
