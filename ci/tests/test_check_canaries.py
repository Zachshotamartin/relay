from __future__ import annotations

import base64
import tempfile
import unittest
from pathlib import Path

from ci import check_canaries


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


if __name__ == "__main__":
    unittest.main()
