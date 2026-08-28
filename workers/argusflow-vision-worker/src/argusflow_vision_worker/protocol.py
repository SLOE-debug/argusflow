"""Framed JSON and Windows Named Pipe transport for the vision worker."""

from __future__ import annotations

import json
import struct
from typing import Any

import pywintypes
import win32file
import win32pipe

PROTOCOL_VERSION = "argusflow.vision.v1"
MAX_FRAME_BYTES = 4 * 1024 * 1024


class ProtocolError(RuntimeError):
    """Raised when a peer sends an invalid or oversized protocol frame."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message


def _read_exact(handle: pywintypes.HANDLE, length: int) -> bytes:
    chunks: list[bytes] = []
    remaining = length
    while remaining:
        _, chunk = win32file.ReadFile(handle, remaining)
        if not chunk:
            raise ProtocolError("pipe_closed", "named pipe closed before a full frame arrived")
        chunks.append(bytes(chunk))
        remaining -= len(chunk)
    return b"".join(chunks)


def read_frame(handle: pywintypes.HANDLE) -> dict[str, Any]:
    """Read one little-endian length-prefixed JSON object."""

    header = _read_exact(handle, 4)
    (length,) = struct.unpack("<I", header)
    if length > MAX_FRAME_BYTES:
        raise ProtocolError("frame_too_large", f"control frame is {length} bytes")
    try:
        payload = _read_exact(handle, length)
        decoded = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ProtocolError("invalid_json", f"invalid control JSON: {error}") from error
    if not isinstance(decoded, dict):
        raise ProtocolError("invalid_message", "control payload must be a JSON object")
    return decoded


def write_frame(handle: pywintypes.HANDLE, message: dict[str, Any]) -> None:
    """Write one little-endian length-prefixed JSON object."""

    payload = json.dumps(
        message,
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")
    if len(payload) > MAX_FRAME_BYTES:
        raise ProtocolError("frame_too_large", f"control frame is {len(payload)} bytes")
    frame = struct.pack("<I", len(payload)) + payload
    win32file.WriteFile(handle, frame)


def create_server(pipe_name: str) -> pywintypes.HANDLE:
    """Create a byte-mode single-instance pipe for the current worker session."""

    return win32pipe.CreateNamedPipe(
        pipe_name,
        win32pipe.PIPE_ACCESS_DUPLEX,
        win32pipe.PIPE_TYPE_BYTE
        | win32pipe.PIPE_READMODE_BYTE
        | win32pipe.PIPE_WAIT,
        1,
        MAX_FRAME_BYTES,
        MAX_FRAME_BYTES,
        0,
        None,
    )


def connect_server(handle: pywintypes.HANDLE) -> None:
    """Block until the Rust client connects to one worker instance."""

    try:
        win32pipe.ConnectNamedPipe(handle, None)
    except pywintypes.error as error:
        if error.winerror != 535:
            raise


def close_server(handle: pywintypes.HANDLE) -> None:
    """Disconnect and close a worker pipe instance."""

    try:
        win32pipe.DisconnectNamedPipe(handle)
    finally:
        win32file.CloseHandle(handle)
