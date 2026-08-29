"""Desktop-text enhancement tests without loading Paddle native models."""

from __future__ import annotations

import unittest

import numpy
import cv2

from argusflow_vision_worker.image_preprocessing import (
    ImagePreprocessingMode,
    prepare_ocr_image,
)
from argusflow_vision_worker.diagnostic_artifact import encode_exact_model_input


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

    def test_exact_model_input_png_round_trips_every_preprocessing_shape(self) -> None:
        """No-op, upscale, CLAHE and sharpen paths must decode to predict pixels exactly."""

        no_op = numpy.arange(80 * 200 * 3, dtype=numpy.uint8).reshape(80, 200, 3)
        upscale = numpy.zeros((60, 120, 3), dtype=numpy.uint8)
        upscale[:, ::2] = 255
        clahe = numpy.full((180, 300, 3), 40, dtype=numpy.uint8)
        clahe[40:140, 60:240] = 100
        sharpen = numpy.full((70, 180, 3), 30, dtype=numpy.uint8)
        sharpen[20:50, 30:150] = 110
        fixtures = [
            prepare_ocr_image(no_op, ImagePreprocessingMode.NONE),
            prepare_ocr_image(upscale, ImagePreprocessingMode.ADAPTIVE_DESKTOP_TEXT),
            prepare_ocr_image(clahe, ImagePreprocessingMode.ADAPTIVE_DESKTOP_TEXT),
            prepare_ocr_image(sharpen, ImagePreprocessingMode.ADAPTIVE_DESKTOP_TEXT),
        ]

        for prepared in fixtures:
            artifact = encode_exact_model_input(prepared.pixels)
            decoded_bgr = cv2.imdecode(
                numpy.frombuffer(artifact.body, dtype=numpy.uint8),
                cv2.IMREAD_COLOR,
            )
            decoded_rgb = cv2.cvtColor(decoded_bgr, cv2.COLOR_BGR2RGB)

            self.assertTrue(numpy.array_equal(decoded_rgb, prepared.pixels))
            self.assertEqual(artifact.width, prepared.output_width)
            self.assertEqual(artifact.height, prepared.output_height)


if __name__ == "__main__":
    unittest.main()
