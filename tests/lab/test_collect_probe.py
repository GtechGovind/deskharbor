import json
import pathlib
import sys
import tempfile
import unittest

from collect_probe import collect, classify_context


class CollectionTest(unittest.TestCase):
    def test_wsl_and_wrong_ubuntu_cannot_qualify_as_linux_lab(self):
        self.assertFalse(classify_context("Linux", "6.8-microsoft-WSL2", "24.04", "x86_64")["eligible_os"])
        self.assertFalse(classify_context("Linux", "6.8-generic", "26.04", "x86_64")["eligible_os"])
        self.assertFalse(classify_context("Linux", "6.8-generic", "24.04", "x86_64", "other_distro")["eligible_os"])
        self.assertFalse(classify_context("Darwin", "22.0", "13.0", "arm64", "macos")["eligible_os"])
        self.assertFalse(classify_context("Windows", "11", "11", "AMD64", "server")["eligible_os"])
        facts = classify_context("Linux", "6.8-generic", "24.04", "x86_64", "ubuntu")
        self.assertTrue(facts["eligible_os"])
        self.assertFalse(facts["hardware_or_session_proven"])

    def test_real_subprocess_evidence_never_becomes_an_unattended_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            destination = pathlib.Path(temporary) / "run"
            result = collect([sys.executable, "-c", "import json; print(json.dumps({'schema_version':1,'platform':'windows','checks':[]}))"], destination)
            self.assertEqual(result["probe_exit"], 0)
            self.assertEqual(result["p0_status"], "not_run")
            self.assertTrue((destination / "inspection.json").is_file())
            self.assertEqual(json.loads((destination / "collection.json").read_text())["p0_status"], "not_run")
            with self.assertRaises(FileExistsError):
                collect([sys.executable, "-c", "print('overwrite')"], destination)

    def test_failed_probe_is_retained_and_cannot_be_reported_as_passed(self):
        with tempfile.TemporaryDirectory() as temporary:
            destination = pathlib.Path(temporary) / "run"
            result = collect([sys.executable, "-c", "import sys; print('denied',file=sys.stderr); sys.exit(7)"], destination)
            self.assertEqual(result["probe_exit"], 7)
            self.assertEqual(result["inspection_status"], "failed")
            self.assertEqual(result["p0_status"], "not_run")
            self.assertIn("denied", (destination / "stderr.txt").read_text())


if __name__ == "__main__":
    unittest.main()
