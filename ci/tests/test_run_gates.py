from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from ci import run_gates


def registry(*, r0_status: str = "accepted", r0_commands: str = '"true"') -> str:
    sections = [
        "schema = 1",
        "",
        "[gate.R0]",
        f'status = "{r0_status}"',
        'section = "BUILD_PLAN.md §5"',
        f"commands = [{r0_commands}]",
    ]
    for number in range(1, 11):
        sections.extend(
            [
                "",
                f"[gate.R{number}]",
                'status = "planned"',
                f'section = "BUILD_PLAN.md §{number + 5}"',
                "commands = []",
            ]
        )
    return "\n".join(sections) + "\n"


class ParseRegistryTests(unittest.TestCase):
    def test_r0_gate_01_parses_all_eleven_gates_in_order(self) -> None:
        parsed = run_gates.parse_registry(registry())

        self.assertEqual(1, parsed.schema)
        self.assertEqual([f"R{number}" for number in range(11)], list(parsed.gates))
        self.assertEqual(("true",), parsed.gates["R0"].commands)

    def test_r0_gate_02_rejects_missing_gate(self) -> None:
        malformed = registry().replace(
            '\n[gate.R10]\nstatus = "planned"\nsection = "BUILD_PLAN.md §15"\ncommands = []\n',
            "\n",
        )

        with self.assertRaisesRegex(run_gates.RegistryError, "R10"):
            run_gates.parse_registry(malformed)

    def test_r0_gate_03_rejects_duplicate_gate(self) -> None:
        malformed = registry() + "\n[gate.R0]\nstatus = \"planned\"\n"

        with self.assertRaisesRegex(run_gates.RegistryError, "duplicate.*R0"):
            run_gates.parse_registry(malformed)

    def test_r0_gate_04_rejects_unknown_status(self) -> None:
        with self.assertRaisesRegex(run_gates.RegistryError, "status"):
            run_gates.parse_registry(registry(r0_status="done"))

    def test_r0_gate_05_rejects_accepted_gate_without_commands(self) -> None:
        with self.assertRaisesRegex(run_gates.RegistryError, "accepted.*commands"):
            run_gates.parse_registry(registry(r0_commands=""))


class ReplayRegistryTests(unittest.TestCase):
    def test_r0_gate_06_replays_only_accepted_commands_in_registry_order(self) -> None:
        parsed = run_gates.parse_registry(registry(r0_commands='"first", "second"'))
        seen: list[tuple[str, Path]] = []

        def runner(command: str, cwd: Path) -> int:
            seen.append((command, cwd))
            return 0

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            replayed = run_gates.replay_accepted(parsed, root, runner=runner)

        self.assertEqual(["first", "second"], replayed)
        self.assertEqual(["first", "second"], [command for command, _ in seen])

    def test_r0_gate_07_stops_at_first_nonzero_command(self) -> None:
        parsed = run_gates.parse_registry(registry(r0_commands='"first", "fails", "never"'))
        seen: list[str] = []

        def runner(command: str, _cwd: Path) -> int:
            seen.append(command)
            return 7 if command == "fails" else 0

        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(run_gates.GateCommandError, "fails.*7"):
                run_gates.replay_accepted(parsed, Path(directory), runner=runner)

        self.assertEqual(["first", "fails"], seen)


if __name__ == "__main__":
    unittest.main()
