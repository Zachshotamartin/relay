from __future__ import annotations

import base64
import contextlib
import io
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from ci import check_canaries


SCRIPT = Path(__file__).resolve().parents[1] / "check_canaries.py"


class CanaryScanTests(unittest.TestCase):
    def test_r0_canary_01_accepts_clean_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test-output.log"
            path.write_bytes(b"all deterministic tests passed\n")

            self.assertEqual([], check_canaries.scan_paths([path]))

    def test_r0_canary_02_detects_raw_canary_without_returning_secret(self) -> None:
        secret = b"RELAY_CANARY_do-not-print-this-value"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test-output.log"
            path.write_bytes(b"prefix " + secret + b" suffix")

            findings = check_canaries.scan_paths([path])

        self.assertEqual(1, len(findings))
        self.assertEqual("raw", findings[0].encoding)
        self.assertNotIn(secret.decode(), repr(findings[0]))

    def test_r0_canary_03_detects_encoded_prefixes(self) -> None:
        raw = b"RELAY_CANARY_secret"
        encoded = base64.b64encode(raw) + b" " + raw.hex().encode("ascii")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "artifact.bin"
            path.write_bytes(encoded)

            encodings = {finding.encoding for finding in check_canaries.scan_paths([path])}

        self.assertEqual({"base64", "hex"}, encodings)

    def test_r0_canary_04_detects_unpadded_urlsafe_base64_and_uppercase_hex(self) -> None:
        raw = b"RELAY_CANARY_\xfb\xff"
        encoded = base64.urlsafe_b64encode(raw).rstrip(b"=")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "artifact.bin"
            path.write_bytes(encoded + b" " + raw.hex().upper().encode("ascii"))

            encodings = {finding.encoding for finding in check_canaries.scan_paths([path])}

        self.assertEqual({"base64", "hex"}, encodings)

    def test_r0_canary_05_requires_the_complete_encoded_prefix(self) -> None:
        partial = b"RELAY_CANARY"
        encoded = base64.b64encode(partial) + b" " + partial.hex().encode("ascii")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "artifact.bin"
            path.write_bytes(encoded)

            self.assertEqual([], check_canaries.scan_paths([path]))

    def test_r0_canary_06_scans_directories_in_deterministic_order(self) -> None:
        raw = b"RELAY_CANARY_secret"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            later = root / "z.bin"
            earlier = root / "nested" / "a.bin"
            earlier.parent.mkdir()
            later.write_bytes(raw)
            earlier.write_bytes(raw.hex().encode("ascii"))

            findings = check_canaries.scan_paths([root])

        self.assertEqual(
            [("a.bin", "hex"), ("z.bin", "raw")],
            [(finding.path.name, finding.encoding) for finding in findings],
        )

    def test_r0_canary_07_reports_each_encoding_once_per_file(self) -> None:
        raw = b"RELAY_CANARY_secret"
        encoded = base64.b64encode(raw)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "artifact.bin"
            path.write_bytes(raw + b" " + raw + b" " + encoded + b" " + encoded)

            findings = check_canaries.scan_paths([path])

        self.assertEqual(["raw", "base64"], [finding.encoding for finding in findings])

    def test_r0_canary_08_fails_closed_on_missing_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            missing = Path(directory) / "missing.log"

            with self.assertRaisesRegex(check_canaries.ScanError, "missing"):
                check_canaries.scan_paths([missing])

    def test_r0_canary_09_cli_diagnostic_never_contains_secret(self) -> None:
        secret = "RELAY_CANARY_do-not-print-this-value"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "captured.log"
            path.write_text(secret, encoding="utf-8")
            stderr = io.StringIO()

            with contextlib.redirect_stderr(stderr):
                status = check_canaries.main([str(path)])

        self.assertEqual(1, status)
        self.assertIn("raw", stderr.getvalue())
        self.assertNotIn(secret, stderr.getvalue())

    def test_r0_canary_10_capture_replays_clean_combined_output_unchanged(self) -> None:
        child = (
            "import os; "
            "os.write(1, b'stdout-bytes\\n'); "
            "os.write(2, b'stderr-bytes\\n')"
        )

        completed = subprocess.run(
            [sys.executable, str(SCRIPT), "--capture", "--", sys.executable, "-c", child],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        self.assertEqual(0, completed.returncode)
        self.assertEqual(b"stdout-bytes\nstderr-bytes\n", completed.stdout)
        self.assertEqual(b"", completed.stderr)

    def test_r0_canary_11_capture_preserves_child_failure_status(self) -> None:
        child = "import os, sys; os.write(2, b'failed cleanly\\n'); sys.exit(7)"

        completed = subprocess.run(
            [sys.executable, str(SCRIPT), "--capture", "--", sys.executable, "-c", child],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        self.assertEqual(7, completed.returncode)
        self.assertEqual(b"failed cleanly\n", completed.stdout)
        self.assertEqual(b"", completed.stderr)

    def test_r0_canary_12_capture_withholds_contaminated_output(self) -> None:
        secret = b"RELAY_CANARY_capture-must-not-leak"
        child = f"import os; os.write(1, {secret!r})"

        completed = subprocess.run(
            [sys.executable, str(SCRIPT), "--capture", "--", sys.executable, "-c", child],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        self.assertEqual(1, completed.returncode)
        self.assertEqual(b"", completed.stdout)
        self.assertIn(b"raw", completed.stderr)
        self.assertNotIn(secret, completed.stderr)


if __name__ == "__main__":
    unittest.main()
