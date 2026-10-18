"""No GPU, systemd session, or memory-exhaustion test required."""
import pathlib
import subprocess
import tempfile
import unittest

GUARD = pathlib.Path(__file__).resolve().with_name("toon-memory-guard.sh")
LIMITS = {
    "memory.high": "3221225472",
    "memory.max": "4294967296",
    "memory.swap.max": "536870912",
    "memory.oom.group": "1",
}


class MemoryGuardTests(unittest.TestCase):
    def verify(self, changes):
        with tempfile.TemporaryDirectory(prefix="toon guard ") as directory:
            for name, value in (LIMITS | changes).items():
                if value is not None:
                    pathlib.Path(directory, name).write_text(value + "\n")
            return subprocess.run(
                ["bash", "-c", 'source "$1"; toon_verify_limits "$2"',
                 "test", str(GUARD), directory],
                capture_output=True, text=True,
            )

    def test_exact_limits_are_accepted(self):
        result = self.verify({})
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_unlimited_ram_is_rejected(self):
        self.assertNotEqual(self.verify({"memory.max": "max"}).returncode, 0)

    def test_missing_controller_is_rejected(self):
        self.assertNotEqual(self.verify({"memory.max": None}).returncode, 0)

    def test_wrong_throttle_is_rejected(self):
        self.assertNotEqual(self.verify({"memory.high": "max"}).returncode, 0)

    def test_unlimited_swap_is_rejected(self):
        self.assertNotEqual(self.verify({"memory.swap.max": "max"}).returncode, 0)

    def test_disabled_group_kill_is_rejected(self):
        self.assertNotEqual(self.verify({"memory.oom.group": "0"}).returncode, 0)

    def test_malformed_limit_is_rejected(self):
        self.assertNotEqual(self.verify({"memory.max": "invalid"}).returncode, 0)


if __name__ == "__main__":
    unittest.main()
