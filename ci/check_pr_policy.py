#!/usr/bin/env python3
"""Validate Relay pull-request titles, template shape, and evidence fields."""

from __future__ import annotations

import argparse
import os
import re
import sys
from collections.abc import Sequence
from pathlib import Path


CONVENTIONAL_TYPES = (
    "feat",
    "fix",
    "refactor",
    "docs",
    "test",
    "chore",
    "perf",
    "ci",
)
TITLE = re.compile(
    rf"^(?:{'|'.join(CONVENTIONAL_TYPES)}): [^\r\n]+$"
)
REQUIRED_HEADINGS = (
    "## Requirements and evidence",
    "## Test-first record",
    "## Artifact changes",
    "## Dependency review",
    "## Format changes",
)
REQUIRED_FIELDS = (
    "Requirement IDs:",
    "Matrix row IDs:",
    "Failing-test commit:",
    "Golden files / corpus entries / fixtures changed:",
    "Semantic reason:",
    "Dependency review checklist:",
    "Version bump:",
    "MIGR fixture:",
)


def validate_title(title: str) -> list[str]:
    """Return policy errors for a pull-request title."""

    if title != title.strip() or TITLE.fullmatch(title) is None:
        return [
            "pull-request title must use conventional form "
            f"<type>: <description>; type is one of {', '.join(CONVENTIONAL_TYPES)}"
        ]
    return []


def validate_template(body: str) -> list[str]:
    """Validate that every fixed template heading and field occurs once."""

    lines = [line.strip() for line in body.splitlines()]
    errors: list[str] = []
    for heading in REQUIRED_HEADINGS:
        count = lines.count(heading)
        if count != 1:
            errors.append(f"pull-request template must contain {heading!r} exactly once")
    for field in REQUIRED_FIELDS:
        count = sum(line.startswith(field) for line in lines)
        if count != 1:
            errors.append(f"pull-request template must contain {field!r} exactly once")
    return errors


def validate_pr_body(body: str) -> list[str]:
    """Validate template structure and require a response for every evidence field."""

    errors = validate_template(body)
    if errors:
        return errors

    without_comments = re.sub(r"<!--.*?-->", "", body, flags=re.DOTALL)
    lines = [line.strip() for line in without_comments.splitlines()]
    for field in REQUIRED_FIELDS:
        index = next(
            line_index
            for line_index, line in enumerate(lines)
            if line.startswith(field)
        )
        response = lines[index][len(field) :].strip()
        cursor = index + 1
        while not response and cursor < len(lines):
            candidate = lines[cursor]
            if candidate.startswith("## ") or any(
                candidate.startswith(other) for other in REQUIRED_FIELDS
            ):
                break
            if candidate:
                response = candidate
                break
            cursor += 1
        if not response:
            errors.append(f"pull-request body must answer {field!r}")
    return errors


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--title", default=os.environ.get("RELAY_PR_TITLE"))
    parser.add_argument("--body", default=os.environ.get("RELAY_PR_BODY"))
    parser.add_argument(
        "--template",
        type=Path,
        default=Path(".github/pull_request_template.md"),
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    errors: list[str] = []
    try:
        template = args.template.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        errors.append(f"cannot read pull-request template: {error}")
    else:
        errors.extend(validate_template(template))

    if args.title is not None:
        errors.extend(validate_title(args.title))
    if args.body is not None:
        errors.extend(validate_pr_body(args.body))
    for error in errors:
        print(f"PR policy: {error}", file=sys.stderr)
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
