"""Command-line entry point for the ArgusFlow PaddleOCR worker."""

from __future__ import annotations

import argparse

from .worker import serve


def main() -> None:
    """Start one worker bound to the caller-provided randomized session pipe."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--pipe-name", required=True)
    parser.add_argument("--session-token", required=True)
    arguments = parser.parse_args()
    serve(arguments.pipe_name, arguments.session_token)


if __name__ == "__main__":
    main()
