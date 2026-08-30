#!/usr/bin/env python3
"""Enforce Relay evidence-family prefixes on Rust integration tests."""

from __future__ import annotations

import argparse
import os
import re
import sys
from collections.abc import Iterator, Sequence
from dataclasses import dataclass
from pathlib import Path


FAMILY_PREFIXES = (
    "core",
    "stor",
    "crsh",
    "sim",
    "modl",
    "fifo",
    "topc",
    "wire",
    "fuzz",
    "raft",
    "admn",
    "opsx",
    "migr",
    "soak",
    "bench",
    "mut",
    "mkt",
)
_IDENTIFIER = r"(?:r#)?[A-Za-z_][A-Za-z0-9_]*"
_ATTRIBUTE_START = re.compile(r"#\s*\[")
_ATTRIBUTE_PATH = re.compile(
    rf"\s*(?P<path>{_IDENTIFIER}(?:\s*::\s*{_IDENTIFIER})*)"
)
_FUNCTION = re.compile(
    rf"(?:pub\s*(?:\([^)]*\))?\s+)?"
    rf"(?:(?:const|async|unsafe|extern)\s+)*"
    rf"fn\s+(?P<name>{_IDENTIFIER})"
)
_FUNCTION_DECLARATION = re.compile(
    r"(?:pub\s*(?:\([^)]*\))?\s+)?"
    r"(?:(?:const|async|unsafe|extern)\s+)*"
    r"fn\b"
)
_TEST_ATTRIBUTE_NAMES = {"test", "rstest", "proptest", "test_case"}


class ScanError(RuntimeError):
    """The convention scan could not inspect its complete input."""


@dataclass(frozen=True)
class Violation:
    """One test function whose name has no approved evidence-family prefix."""

    path: Path
    line: int
    name: str


def _mask_range(characters: list[str], start: int, end: int) -> None:
    for index in range(start, end):
        if characters[index] not in {"\n", "\r"}:
            characters[index] = " "


def _raw_string_end(source: str, start: int) -> int | None:
    if start > 0 and (source[start - 1].isalnum() or source[start - 1] == "_"):
        return None
    if source.startswith("br", start):
        cursor = start + 2
    elif source.startswith("r", start):
        cursor = start + 1
    else:
        return None
    hashes = 0
    while cursor < len(source) and source[cursor] == "#":
        hashes += 1
        cursor += 1
    if cursor >= len(source) or source[cursor] != '"':
        return None
    terminator = '"' + ("#" * hashes)
    end = source.find(terminator, cursor + 1)
    return len(source) if end < 0 else end + len(terminator)


def _quoted_end(source: str, start: int, quote: str) -> int:
    cursor = start + 1
    escaped = False
    while cursor < len(source):
        character = source[cursor]
        if escaped:
            escaped = False
        elif character == "\\":
            escaped = True
        elif character == quote:
            return cursor + 1
        cursor += 1
    return len(source)


def _char_literal_end(source: str, start: int) -> int | None:
    cursor = start + 1
    if cursor >= len(source) or source[cursor] in {"\n", "\r", "'"}:
        return None
    escaped = False
    while cursor < len(source) and source[cursor] not in {"\n", "\r"}:
        character = source[cursor]
        if escaped:
            escaped = False
        elif character == "\\":
            escaped = True
        elif character == "'":
            return cursor + 1
        cursor += 1
    return None


def _mask_noncode(source: str) -> str:
    """Blank comments and literals while preserving byte positions and lines."""

    characters = list(source)
    cursor = 0
    while cursor < len(source):
        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            end = len(source) if end < 0 else end
            _mask_range(characters, cursor, end)
            cursor = end
            continue
        if source.startswith("/*", cursor):
            depth = 1
            end = cursor + 2
            while end < len(source) and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            _mask_range(characters, cursor, end)
            cursor = end
            continue

        raw_end = _raw_string_end(source, cursor)
        if raw_end is not None:
            _mask_range(characters, cursor, raw_end)
            cursor = raw_end
            continue
        if source[cursor] == '"':
            end = _quoted_end(source, cursor, '"')
            _mask_range(characters, cursor, end)
            cursor = end
            continue
        if source[cursor] == "'":
            end = _char_literal_end(source, cursor)
            if end is not None:
                _mask_range(characters, cursor, end)
                cursor = end
                continue
        cursor += 1
    return "".join(characters)


def _matching_bracket(source: str, opening: int) -> int | None:
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "[":
            depth += 1
        elif source[index] == "]":
            depth -= 1
            if depth == 0:
                return index
    return None


def _top_level_arguments(contents: str) -> list[str] | None:
    contents = contents.strip()
    if not contents.startswith("("):
        return None
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[str] = []
    arguments: list[str] = []
    start = 1
    for index, character in enumerate(contents):
        if character in pairs:
            stack.append(pairs[character])
        elif character in pairs.values():
            if not stack or stack.pop() != character:
                return None
            if not stack:
                if contents[index + 1 :].strip():
                    return None
                arguments.append(contents[start:index].strip())
                return arguments
        elif character == "," and len(stack) == 1:
            arguments.append(contents[start:index].strip())
            start = index + 1
    return None


def _is_test_attribute(contents: str) -> bool:
    match = _ATTRIBUTE_PATH.match(contents)
    if match is None:
        return False
    segments = re.findall(_IDENTIFIER, match.group("path"))
    if not segments:
        return False
    attribute_name = segments[-1].removeprefix("r#")
    remainder = contents[match.end() :].lstrip()
    if attribute_name in _TEST_ATTRIBUTE_NAMES:
        return not remainder or _top_level_arguments(remainder) is not None
    if attribute_name != "cfg_attr":
        return False
    arguments = _top_level_arguments(remainder)
    if arguments is None or len(arguments) < 2:
        return False
    return any(_is_test_attribute(argument) for argument in arguments[1:])


def _skip_attributes(source: str, cursor: int) -> int:
    while True:
        cursor += len(source[cursor:]) - len(source[cursor:].lstrip())
        match = _ATTRIBUTE_START.match(source, cursor)
        if match is None:
            return cursor
        opening = source.find("[", cursor, match.end())
        closing = _matching_bracket(source, opening)
        if closing is None:
            return len(source)
        cursor = closing + 1


def _test_functions(source: str) -> Iterator[tuple[str, int]]:
    masked = _mask_noncode(source)
    seen: set[tuple[str, int]] = set()
    for attribute in _ATTRIBUTE_START.finditer(masked):
        opening = masked.find("[", attribute.start(), attribute.end())
        closing = _matching_bracket(masked, opening)
        if closing is None or not _is_test_attribute(masked[opening + 1 : closing]):
            continue
        function_start = _skip_attributes(masked, closing + 1)
        function = _FUNCTION.match(masked, function_start)
        if function is None:
            if _FUNCTION_DECLARATION.match(masked, function_start) is not None:
                line = source.count("\n", 0, attribute.start()) + 1
                key = ("<dynamic-test-name>", line)
                if key not in seen:
                    seen.add(key)
                    yield key
            continue
        name = function.group("name").removeprefix("r#")
        line = source.count("\n", 0, function.start("name")) + 1
        key = (name, line)
        if key not in seen:
            seen.add(key)
            yield key


def _rust_test_files(root: Path) -> list[Path]:
    try:
        if root.is_symlink():
            raise ScanError(f"refusing symbolic-link scan root: {root}")
        if root.is_file():
            return [root] if root.suffix == ".rs" else []
        if not root.is_dir():
            raise ScanError(f"scan root is missing or not a directory: {root}")
    except OSError as error:
        raise ScanError(f"cannot inspect scan root: {root}") from error

    files: list[Path] = []

    def traversal_error(error: OSError) -> None:
        raise ScanError(f"cannot traverse test tree below: {root}") from error

    for directory, directory_names, file_names in os.walk(
        root, topdown=True, followlinks=False, onerror=traversal_error
    ):
        directory_names.sort()
        file_names.sort()
        directory_path = Path(directory)
        for name in tuple(directory_names):
            candidate = directory_path / name
            if candidate.is_symlink():
                raise ScanError(f"refusing symbolic-link directory: {candidate}")
        for name in file_names:
            candidate = directory_path / name
            if candidate.is_symlink():
                raise ScanError(f"refusing symbolic-link file: {candidate}")
            relative_parts = candidate.relative_to(root).parts[:-1]
            in_tests_tree = root.name == "tests" or "tests" in relative_parts
            if in_tests_tree and candidate.suffix == ".rs":
                files.append(candidate)
    return files


def find_violations(root: Path) -> list[Violation]:
    """Return every misnamed Rust test below a directory named ``tests``."""

    violations: list[Violation] = []
    for path in _rust_test_files(root):
        try:
            source = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as error:
            raise ScanError(f"Rust source is not valid UTF-8: {path}") from error
        except OSError as error:
            raise ScanError(f"cannot read Rust source: {path}") from error
        for name, line in _test_functions(source):
            if not any(name.startswith(f"{family}_") for family in FAMILY_PREFIXES):
                violations.append(Violation(path=path, line=line, name=name))
    return sorted(violations, key=lambda item: (str(item.path), item.line, item.name))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "root",
        nargs="?",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository tree to scan (default: repository root)",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        violations = find_violations(args.root)
    except ScanError as error:
        print(f"test-name scan failed closed: {error}", file=sys.stderr)
        return 2
    for violation in violations:
        families = ", ".join(f"{family}_" for family in FAMILY_PREFIXES)
        print(
            f"{violation.path}:{violation.line}: test {violation.name!r} must start "
            f"with an evidence-family prefix ({families})",
            file=sys.stderr,
        )
    return 1 if violations else 0


if __name__ == "__main__":
    raise SystemExit(main())
