"""Desktop-text enhancement tests without loading Paddle native models."""

from __future__ import annotations

import unittest

import numpy

from argusflow_vision_worker.image_preprocessing import (
    ImagePreprocessingMode,
    prepare_ocr_image,
)


class ImagePreprocessingTests(unittest.TestCase):
    """Verify bounded enhancement and reversible ROI geometry."""

    def test_none_preserves_pixels_and_geometry(self) -> None:
        image = numpy.full((80, 200, 3), 42, dtype=numpy.uint8)

        prepared = prepare_ocr_image(image, ImagePreprocessingMode.NONE)

        self.assertIs(prepared.pixels, image)
        self.assertEqual(prepared.summary()["output_width"], 200)
        self.assertFalse(prepared.sharpened)

    def test_small_dark_roi_is_enlarged_and_maps_coordinates_back(self) -> None:
        image = numpy.full((80, 200, 3), 40, dtype=numpy.uint8)
        image[30:50, 40:160] = 110

        prepared = prepare_ocr_image(image, ImagePreprocessingMode.ADAPTIVE_DESKTOP_TEXT)

        self.assertEqual((prepared.output_width, prepared.output_height), (400, 160))
        self.assertTrue(prepared.contrast_enhanced)
        self.assertTrue(prepared.sharpened)
        self.assertAlmostEqual(prepared.map_x_to_input(200.0), 100.0)
        self.assertAlmostEqual(prepared.map_y_to_input(80.0), 40.0)

    def test_large_frame_does_not_multiply_inference_pixels(self) -> None:
        image = numpy.full((700, 1_000, 3), 128, dtype=numpy.uint8)

        prepared = prepare_ocr_image(image, ImagePreprocessingMode.ADAPTIVE_DESKTOP_TEXT)

        self.assertEqual((prepared.output_width, prepared.output_height), (1_000, 700))
        self.assertFalse(prepared.contrast_enhanced)
        self.assertFalse(prepared.sharpened)


if __name__ == "__main__":
    unittest.main()
