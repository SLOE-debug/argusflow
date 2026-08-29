"""Framed JSON and Windows Named Pipe transport for the vision worker."""

from __future__ import annotations

import json
import struct
from typing import Any

import pywintypes
import win32file
import win32pipe

PROTOCOL_VERSION = "argusflow.vision.v4"
MAX_CONTROL_FRAME_BYTES = 4 * 1024 * 1024
MAX_PIXEL_BODY_BYTES = 64 * 1024 * 1024
FRAME_MAGIC = b"AFV2"
FRAME_HEADER = struct.Struct("<4sIQ")


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


def read_frame(handle: pywintypes.HANDLE) -> tuple[dict[str, Any], bytes]:
    """Read one control JSON object followed by an optional binary pixel body."""

    header = _read_exact(handle, FRAME_HEADER.size)
    magic, control_length, body_length = FRAME_HEADER.unpack(header)
    if magic != FRAME_MAGIC:
        raise ProtocolError("invalid_frame", "vision worker frame magic mismatch")
    if control_length > MAX_CONTROL_FRAME_BYTES:
        raise ProtocolError("frame_too_large", f"control frame is {control_length} bytes")
    if body_length > MAX_PIXEL_BODY_BYTES:
        raise ProtocolError("body_too_large", f"binary body is {body_length} bytes")
    try:
        payload = _read_exact(handle, control_length)
        decoded = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ProtocolError("invalid_json", f"invalid control JSON: {error}") from error
    if not isinstance(decoded, dict):
        raise ProtocolError("invalid_message", "control payload must be a JSON object")
    return decoded, _read_exact(handle, body_length)


def write_frame(handle: pywintypes.HANDLE, message: dict[str, Any], body: bytes = b"") -> None:
    """Write one control JSON object followed by an optional binary body."""

    payload = json.dumps(
        message,
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")
    if len(payload) > MAX_CONTROL_FRAME_BYTES:
        raise ProtocolError("frame_too_large", f"control frame is {len(payload)} bytes")
    if len(body) > MAX_PIXEL_BODY_BYTES:
        raise ProtocolError("body_too_large", f"binary body is {len(body)} bytes")
    frame = FRAME_HEADER.pack(FRAME_MAGIC, len(payload), len(body)) + payload + body
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
        MAX_CONTROL_FRAME_BYTES,
        MAX_CONTROL_FRAME_BYTES,
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
