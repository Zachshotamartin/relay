#!/usr/bin/env python3
"""Fail when Relay's secret-canary marker appears in captured artifacts."""

from __future__ import annotations

import argparse
import base64
import os
import re
import stat
import subprocess
import sys
import tempfile
import urllib.parse
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path


CANARY_PREFIX = b"RELAY_CANARY_"
_CANARY_TEXT = CANARY_PREFIX.decode("ascii")
_BASE64_TOKEN = re.compile(rb"[A-Za-z0-9+/_-]{8,}={0,2}")
_HEX_PREFIX = CANARY_PREFIX.hex().encode("ascii")
_ENCODING_ORDER = {"raw": 0, "base64": 1, "hex": 2, "url": 3}


class ScanError(RuntimeError):
    """The canary scan could not inspect its complete input."""


def _safe_text(value: object) -> str:
    text = str(value)
    marker = text.find(_CANARY_TEXT)
    if marker >= 0:
        return f"{text[:marker]}<redacted-canary>"
    components = re.split(r"[/\\]", text)
    if any(
        _encodings(component.encode("utf-8", errors="surrogateescape"))
        for component in components
    ):
        return "<redacted-canary-path>"
    return text


@dataclass(frozen=True)
class Finding:
    """A canary encoding found in one file; matched bytes are never retained."""

    path: Path
    encoding: str

    def __repr__(self) -> str:
        return f"Finding(path={_safe_text(self.path)!r}, encoding={self.encoding!r})"


def _directory_files(root: Path) -> list[Path]:
    files: list[Path] = []

    def traversal_error(error: OSError) -> None:
        raise ScanError(f"cannot traverse directory: {_safe_text(root)}") from error

    for directory, directory_names, file_names in os.walk(
        root, topdown=True, followlinks=False, onerror=traversal_error
    ):
        directory_names.sort()
        file_names.sort()
        directory_path = Path(directory)
        for name in tuple(directory_names):
            candidate = directory_path / name
            if candidate.is_symlink():
                raise ScanError(
                    f"refusing symbolic-link directory: {_safe_text(candidate)}"
                )
        for name in file_names:
            candidate = directory_path / name
            if candidate.is_symlink():
                raise ScanError(f"refusing symbolic-link file: {_safe_text(candidate)}")
            try:
                mode = candidate.stat().st_mode
            except OSError as error:
                raise ScanError(f"cannot inspect file: {_safe_text(candidate)}") from error
            if not stat.S_ISREG(mode):
                raise ScanError(f"refusing non-regular file: {_safe_text(candidate)}")
            files.append(candidate)
    return files


def _input_files(paths: Sequence[Path]) -> list[Path]:
    files: dict[str, Path] = {}
    for path in paths:
        try:
            if path.is_symlink():
                raise ScanError(f"refusing symbolic-link input: {_safe_text(path)}")
            mode = path.stat().st_mode
        except FileNotFoundError as error:
            raise ScanError(f"input path is missing: {_safe_text(path)}") from error
        except OSError as error:
            raise ScanError(f"cannot inspect input path: {_safe_text(path)}") from error
        if stat.S_ISDIR(mode):
            candidates = _directory_files(path)
        elif stat.S_ISREG(mode):
            candidates = [path]
        else:
            raise ScanError(f"refusing non-regular input: {_safe_text(path)}")
        for candidate in candidates:
            files[str(candidate.absolute())] = candidate
    return [files[key] for key in sorted(files)]


def _contains_base64_canary(data: bytes) -> bool:
    for match in _BASE64_TOKEN.finditer(data):
        candidate = match.group(0)
        padded = candidate + (b"=" * (-len(candidate) % 4))
        try:
            decoded = base64.b64decode(padded, altchars=b"-_", validate=True)
        except (ValueError, base64.binascii.Error):
            continue
        if CANARY_PREFIX in decoded:
            return True
    return False


def _encodings(data: bytes) -> list[str]:
    encodings: list[str] = []
    if CANARY_PREFIX in data:
        encodings.append("raw")
    compact_base64 = re.sub(rb"[\t\n\r ]+", b"", data)
    if _contains_base64_canary(data) or _contains_base64_canary(compact_base64):
        encodings.append("base64")
    compact_hex = re.sub(rb"[\t\n\r :,_-]+", b"", data)
    if re.search(re.escape(_HEX_PREFIX), compact_hex, flags=re.IGNORECASE) is not None:
        encodings.append("hex")
    decoded_url = urllib.parse.unquote_to_bytes(data)
    if decoded_url != data and CANARY_PREFIX in decoded_url:
        encodings.append("url")
    return encodings


def scan_paths(paths: Sequence[Path]) -> list[Finding]:
    """Scan files or directory trees without retaining matching secret bytes."""

    findings: list[Finding] = []
    for path in _input_files(paths):
        try:
            data = path.read_bytes()
        except OSError as error:
            raise ScanError(f"cannot read input file: {_safe_text(path)}") from error
        findings.extend(Finding(path=path, encoding=encoding) for encoding in _encodings(data))
    return sorted(
        findings,
        key=lambda finding: (str(finding.path), _ENCODING_ORDER[finding.encoding]),
    )


def _normalized_child_status(returncode: int) -> int:
    return returncode if returncode >= 0 else 128 + (-returncode)


def capture_command(command: Sequence[str]) -> int:
    """Capture combined output, forwarding it only after a clean scan."""

    if not command:
        raise ScanError("capture mode requires a command after --")
    try:
        with tempfile.SpooledTemporaryFile(max_size=8 * 1024 * 1024) as captured:
            with subprocess.Popen(
                list(command),
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
            ) as process:
                if process.stdout is None:
                    raise ScanError("capture command did not expose an output pipe")
                while True:
                    chunk = process.stdout.read(64 * 1024)
                    if not chunk:
                        break
                    captured.write(chunk)
                returncode = process.wait()
            captured.seek(0)
            output = captured.read()
    except OSError as error:
        program = _safe_text(command[0])
        raise ScanError(f"cannot execute or capture command: {program}") from error

    encodings = _encodings(output)
    if encodings:
        for encoding in encodings:
            print(
                f"captured command output: {encoding} canary detected; value withheld",
                file=sys.stderr,
            )
        return _normalized_child_status(returncode) if returncode else 1

    sys.stdout.buffer.write(output)
    sys.stdout.buffer.flush()
    return _normalized_child_status(returncode)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--capture",
        action="store_true",
        help="run a command, scan its combined output, and forward only clean output",
    )
    parser.add_argument(
        "arguments",
        nargs=argparse.REMAINDER,
        help="input paths, or a command following -- in capture mode",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    arguments = args.arguments
    if arguments and arguments[0] == "--":
        arguments = arguments[1:]
    try:
        if args.capture:
            return capture_command(arguments)
        if not arguments:
            raise ScanError("at least one input path is required")
        findings = scan_paths([Path(argument) for argument in arguments])
    except ScanError as error:
        print(f"canary scan failed closed: {error}", file=sys.stderr)
        return 2
    for finding in findings:
        print(
            f"{_safe_text(finding.path)}: {finding.encoding} canary detected; value withheld",
            file=sys.stderr,
        )
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
