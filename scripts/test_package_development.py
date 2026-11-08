"""Checks that development packaging cannot silently ship incomplete bundles."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "package_development", Path(__file__).with_name("package-development.py")
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PackagingTests(unittest.TestCase):
    def test_missing_engine_library_rejects_bundle(self):
        with tempfile.TemporaryDirectory() as root:
            bundle = Path(root)
            (bundle / "deskharbor.exe").write_bytes(b"runner")
            with self.assertRaisesRegex(ValueError, "dh_ffi.dll"):
                module.validate_bundle(bundle, "windows-x64")

    def test_bundle_symlink_cannot_escape_to_private_material(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            bundle = base / "bundle"
            bundle.mkdir()
            (bundle / "deskharbor").write_bytes(b"runner")
            (bundle / "lib").mkdir()
            (bundle / "lib/libdh_ffi.so").write_bytes(b"engine")
            secret = base / "secret"
            secret.write_bytes(b"private")
            try:
                (bundle / "escape").symlink_to(secret)
            except OSError:
                self.skipTest("symlink creation unavailable")
            with self.assertRaisesRegex(ValueError, "outside"):
                module.validate_bundle(bundle, "linux-x64")

    def test_linux_bundle_requires_flutter_runtime_and_assets(self):
        with tempfile.TemporaryDirectory() as root:
            bundle = Path(root)
            (bundle / "deskharbor").write_bytes(b"runner")
            (bundle / "lib").mkdir()
            (bundle / "lib/libdh_ffi.so").write_bytes(b"engine")
            with self.assertRaisesRegex(ValueError, "libflutter_linux_gtk.so"):
                module.validate_bundle(bundle, "linux-x64")

    def test_macos_framework_internal_symlink_is_preserved(self):
        with tempfile.TemporaryDirectory() as root:
            bundle = Path(root) / "deskharbor.app"
            for relative in module.REQUIRED["macos-arm64"]:
                target = bundle / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                if relative.endswith("/"):
                    target.mkdir(exist_ok=True)
                else:
                    target.write_bytes(b"fixture")
            framework = bundle / "Contents/Frameworks/Fixture.framework"
            (framework / "Versions/A").mkdir(parents=True)
            (framework / "Versions/A/Fixture").write_bytes(b"framework")
            try:
                (framework / "Versions/Current").symlink_to("A", target_is_directory=True)
                (framework / "Fixture").symlink_to("Versions/Current/Fixture")
            except OSError:
                self.skipTest("symlink creation unavailable")
            module.validate_bundle(bundle, "macos-arm64")


if __name__ == "__main__":
    unittest.main()
