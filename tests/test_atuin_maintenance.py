"""Offline tests for the fork's helper scripts, requiring no Atuin daemon."""
import importlib.util
import pathlib
import struct
import tempfile
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[1]

def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

class DoctorTests(unittest.TestCase):
    def test_release_uses_release_field_not_build(self):
        module = load("atuin_doctor")
        self.assertEqual(module.release({"ok": True, "stdout": "atuin 18.24.1 (NO_GIT)"}), "18.24.1")
        self.assertIsNone(module.release({"ok": False}))

    def test_missing_executable_is_explicit(self):
        module = load("atuin_doctor")
        self.assertFalse(module.version("/this/file/does/not/exist").get("ok"))

class InstallerTests(unittest.TestCase):
    def test_activation_and_rollback_refuse_missing_previous(self):
        module = load("atuin_install")
        with tempfile.TemporaryDirectory() as t:
            with mock.patch.object(module, "RECORD", pathlib.Path(t) / "record.json"):
                with self.assertRaises(ValueError):
                    module.rollback()

    def test_activation_refuses_existing_nonsymlink(self):
        module = load("atuin_install")
        with tempfile.TemporaryDirectory() as t:
            home = pathlib.Path(t)
            candidate = home / "candidate"
            candidate.write_text("#!/bin/sh\necho atuin 18.24.1\n")
            candidate.chmod(0o755)
            dest = home / "active"
            dest.write_text("do not overwrite")
            with mock.patch.object(module, "DEST", dest), mock.patch.object(module, "RECORD", home / "record.json"):
                with self.assertRaisesRegex(ValueError, "non-symlink"):
                    module.activate(candidate)
            self.assertEqual(dest.read_text(), "do not overwrite")

if __name__ == "__main__":
    unittest.main()
