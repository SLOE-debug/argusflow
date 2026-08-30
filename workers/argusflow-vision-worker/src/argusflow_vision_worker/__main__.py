"""Command-line entry point for the ArgusFlow PaddleOCR worker."""

from __future__ import annotations

import argparse
import os

from .worker import serve


def main() -> None:
    """Start one worker bound to the caller-provided randomized session pipe."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--pipe-name", required=True)
    parser.add_argument("--session-token", required=True)
    parser.add_argument("--status-file")
    parser.add_argument("--device", choices=("auto", "cpu", "gpu:0"), default="auto")
    arguments = parser.parse_args()
    os.environ["ARGUSFLOW_PADDLE_DEVICE"] = arguments.device
    serve(arguments.pipe_name, arguments.session_token, arguments.status_file)


if __name__ == "__main__":
    main()
