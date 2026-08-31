#!/usr/bin/env python3
"""Validate Relay's fixed gate registry and replay every accepted gate."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path


STATUSES = frozenset({"accepted", "in progress", "planned", "deferred"})
EXPECTED_GATES = tuple(f"R{number}" for number in range(11))
HEADER = re.compile(r"^\[gate\.(R(?:10|[0-9]))\]$")
ASSIGNMENT = re.compile(r"^([A-Za-z][A-Za-z0-9_-]*)\s*=\s*(.*)$")


class RegistryError(ValueError):
    """The checked-in gate registry does not satisfy its frozen schema."""


class GateCommandError(RuntimeError):
    """An accepted gate command exited unsuccessfully."""


@dataclass(frozen=True)
class Gate:
    """One validated release-gate record."""

    status: str
    section: str
    commands: tuple[str, ...]


@dataclass(frozen=True)
class Registry:
    """The validated schema version and ordered release-gate records."""

    schema: int
    gates: dict[str, Gate]


def _without_comment(line: str) -> str:
    """Remove a TOML line comment while respecting double-quoted strings."""

    escaped = False
    quoted = False
    for index, character in enumerate(line):
        if quoted:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                quoted = False
        elif character == '"':
            quoted = True
        elif character == "#":
            return line[:index]
    return line


def _parse_string(value: str, *, field: str, line: int) -> str:
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError as error:
        raise RegistryError(f"line {line}: {field} must be a quoted string") from error
    if not isinstance(parsed, str):
        raise RegistryError(f"line {line}: {field} must be a string")
    return parsed


def _array_is_closed(value: str) -> bool:
    escaped = False
    quoted = False
    depth = 0
    for character in value:
        if quoted:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                quoted = False
        elif character == '"':
            quoted = True
        elif character == "[":
            depth += 1
        elif character == "]":
            depth -= 1
            if depth < 0:
                return False
    return depth == 0 and not quoted


def _parse_commands(value: str, *, line: int) -> tuple[str, ...]:
    normalized = value.rstrip()
    if normalized.endswith("]"):
        separator = len(normalized) - 2
        while separator >= 0 and normalized[separator].isspace():
            separator -= 1
        if separator >= 0 and normalized[separator] == ",":
            normalized = normalized[:separator] + normalized[separator + 1 :]
    try:
        parsed = json.loads(normalized)
    except json.JSONDecodeError as error:
        raise RegistryError(f"line {line}: commands must be an array of strings") from error
    if not isinstance(parsed, list) or not all(isinstance(item, str) for item in parsed):
        raise RegistryError(f"line {line}: commands must be an array of strings")
    if any(not item.strip() or "\n" in item or "\r" in item for item in parsed):
        raise RegistryError(f"line {line}: commands must be nonempty single-line strings")
    return tuple(parsed)


def _logical_lines(source: str) -> list[tuple[int, str]]:
    physical_lines = source.splitlines()
    logical_lines: list[tuple[int, str]] = []
    index = 0
    while index < len(physical_lines):
        line_number = index + 1
        line = _without_comment(physical_lines[index]).strip()
        index += 1
        if not line:
            continue
        if line.startswith("commands") and "=" in line:
            while not _array_is_closed(line.partition("=")[2]):
                if index >= len(physical_lines):
                    raise RegistryError(f"line {line_number}: unterminated commands array")
                continuation = _without_comment(physical_lines[index]).strip()
                index += 1
                line = f"{line} {continuation}"
        logical_lines.append((line_number, line))
    return logical_lines


def parse_registry(source: str) -> Registry:
    """Parse and fully validate the fixed `ci/gates.toml` schema."""

    schema: int | None = None
    gate_fields: dict[str, dict[str, tuple[int, str]]] = {}
    order: list[str] = []
    current_gate: str | None = None

    for line_number, line in _logical_lines(source):
        header_match = HEADER.fullmatch(line)
        if header_match:
            gate_id = header_match.group(1)
            if gate_id in gate_fields:
                raise RegistryError(f"line {line_number}: duplicate gate {gate_id}")
            current_gate = gate_id
            gate_fields[gate_id] = {}
            order.append(gate_id)
            continue

        assignment_match = ASSIGNMENT.fullmatch(line)
        if assignment_match is None:
            raise RegistryError(f"line {line_number}: unsupported registry syntax")
        key, value = assignment_match.groups()
        if current_gate is None:
            if key != "schema" or schema is not None:
                raise RegistryError(f"line {line_number}: only one top-level schema is allowed")
            if value != "1":
                raise RegistryError(f"line {line_number}: schema must be 1")
            schema = 1
            continue

        fields = gate_fields[current_gate]
        if key not in {"status", "section", "commands"}:
            raise RegistryError(
                f"line {line_number}: unknown field {key!r} in gate {current_gate}"
            )
        if key in fields:
            raise RegistryError(
                f"line {line_number}: duplicate field {key!r} in gate {current_gate}"
            )
        fields[key] = (line_number, value)

    if schema is None:
        raise RegistryError("missing schema = 1")
    missing = [gate_id for gate_id in EXPECTED_GATES if gate_id not in gate_fields]
    if missing:
        raise RegistryError(f"missing gate sections: {', '.join(missing)}")
    if tuple(order) != EXPECTED_GATES:
        raise RegistryError("gate sections must appear exactly once in R0 through R10 order")

    gates: dict[str, Gate] = {}
    first_unaccepted: str | None = None
    in_progress: str | None = None
    for number, gate_id in enumerate(EXPECTED_GATES):
        fields = gate_fields[gate_id]
        absent = {"status", "section", "commands"} - fields.keys()
        if absent:
            raise RegistryError(
                f"gate {gate_id} is missing fields: {', '.join(sorted(absent))}"
            )

        status_line, status_value = fields["status"]
        status = _parse_string(status_value, field="status", line=status_line)
        if status not in STATUSES:
            raise RegistryError(f"line {status_line}: invalid status {status!r}")

        section_line, section_value = fields["section"]
        section = _parse_string(section_value, field="section", line=section_line)
        expected_section = f"BUILD_PLAN.md §{number + 5}"
        if section != expected_section:
            raise RegistryError(
                f"line {section_line}: gate {gate_id} section must be {expected_section!r}"
            )

        command_line, command_value = fields["commands"]
        commands = _parse_commands(command_value, line=command_line)
        if status == "accepted" and not commands:
            raise RegistryError(f"accepted gate {gate_id} must contain commands")
        if status == "accepted" and first_unaccepted is not None:
            raise RegistryError(
                f"accepted gate {gate_id} follows unaccepted gate {first_unaccepted}"
            )
        if status == "in progress":
            if in_progress is not None:
                raise RegistryError(
                    f"gate {gate_id} is in progress while gate {in_progress} is in progress"
                )
            if first_unaccepted is not None:
                raise RegistryError(
                    f"gate {gate_id} cannot be in progress before {first_unaccepted} is accepted"
                )
            in_progress = gate_id
        if status != "accepted" and first_unaccepted is None:
            first_unaccepted = gate_id

        gates[gate_id] = Gate(status=status, section=section, commands=commands)

    return Registry(schema=schema, gates=gates)


Runner = Callable[[str, Path], int]


def _subprocess_runner(command: str, cwd: Path) -> int:
    completed = subprocess.run(
        command,
        cwd=cwd,
        shell=True,
        executable="/bin/bash",
        check=False,
    )
    return completed.returncode


def replay_accepted(
    registry: Registry,
    cwd: Path,
    *,
    runner: Runner = _subprocess_runner,
) -> list[str]:
    """Replay accepted commands in gate and command order, failing closed."""

    replayed: list[str] = []
    for gate_id, gate in registry.gates.items():
        if gate.status != "accepted":
            continue
        for command in gate.commands:
            print(f"gate replay [{gate_id}]: {command}", flush=True)
            exit_code = runner(command, cwd)
            replayed.append(command)
            if exit_code != 0:
                raise GateCommandError(
                    f"accepted gate command {command!r} exited with status {exit_code}"
                )
    return replayed


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--registry",
        type=Path,
        default=Path(__file__).resolve().parent / "gates.toml",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate without executing accepted commands",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        registry_path = args.registry.resolve()
        registry = parse_registry(registry_path.read_text(encoding="utf-8"))
        accepted = [
            gate_id for gate_id, gate in registry.gates.items() if gate.status == "accepted"
        ]
        print(
            "gate registry valid; accepted gates: "
            + (", ".join(accepted) if accepted else "none")
        )
        if not args.check:
            replay_accepted(registry, registry_path.parent.parent)
    except (OSError, RegistryError, GateCommandError) as error:
        print(f"gate replay failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
