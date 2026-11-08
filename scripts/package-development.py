#!/usr/bin/env python3
"""Package an explicit built bundle. Never read home directories or signing keys."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import tarfile
import tempfile
import zipfile


REQUIRED = {
    "windows-x64": ["deskharbor.exe", "dh_ffi.dll", "flutter_windows.dll", "data/flutter_assets/"],
    "linux-x64": ["deskharbor", "lib/libdh_ffi.so", "lib/libflutter_linux_gtk.so", "data/flutter_assets/"],
    "macos-arm64": ["Contents/MacOS/deskharbor", "Contents/MacOS/dh-hostd", "Contents/MacOS/dh-cli", "Contents/Frameworks/libdh_ffi.dylib", "Contents/Info.plist"],
    "macos-x64": ["Contents/MacOS/deskharbor", "Contents/MacOS/dh-hostd", "Contents/MacOS/dh-cli", "Contents/Frameworks/libdh_ffi.dylib", "Contents/Info.plist"],
}


def validate_bundle(bundle, platform):
    bundle = Path(bundle).resolve(strict=True)
    # Check escapes first so even an incomplete tree cannot hide unsafe links.
    for parent, directories, files in os.walk(bundle, followlinks=False):
        for name in directories + files:
            item = Path(parent) / name
            if item.is_symlink():
                try:
                    item.resolve(strict=True).relative_to(bundle)
                except (ValueError, FileNotFoundError, RuntimeError) as error:
                    raise ValueError(f"bundle symlink points outside bundle or is broken: {item}") from error
                if platform == "windows-x64":
                    raise ValueError(f"Windows bundle must not contain symlinks: {item}")
    for relative in REQUIRED[platform]:
        item = bundle / relative
        valid = item.is_dir() if relative.endswith("/") else item.is_file() and item.stat().st_size > 0
        if not valid:
            raise ValueError(f"missing or empty built bundle component: {relative}")


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=[*REQUIRED, "android-arm64"], required=True)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--binaries", type=Path)
    parser.add_argument("--native-source", type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[a-f0-9]{40}", args.revision):
        parser.error("revision must be a full lowercase Git commit ID")
    repo = Path(__file__).resolve().parent.parent
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    stem = f"DeskHarbor-{args.platform}-dev-{args.revision[:12]}"
    if args.platform == "android-arm64":
        if not args.bundle.is_file() or args.bundle.stat().st_size == 0:
            parser.error("signed APK missing")
    else:
        validate_bundle(args.bundle, args.platform)
        if not args.platform.startswith("macos-") and not args.binaries:
            parser.error("desktop package requires built host/CLI directory")
    with tempfile.TemporaryDirectory(prefix="deskharbor-package-", dir=output.parent) as temporary:
        root = Path(temporary) / stem
        root.mkdir()
        if args.platform == "android-arm64":
            shutil.copy2(args.bundle, root / "DeskHarbor.apk")
            shutil.copy2(repo / "packaging/android/deskharbor-release.cer", root / "android-signing.cer")
        else:
            name = "deskharbor.app" if args.platform.startswith("macos-") else "app"
            shutil.copytree(args.bundle, root / name, symlinks=True)
            binary_dir = root / name
            suffix = ".exe" if args.platform == "windows-x64" else ""
            for binary in ("dh-hostd", "dh-cli"):
                # A signed Mac app is immutable. Its helpers must already be
                # embedded and signed before validation; do not overwrite them.
                if args.platform.startswith("macos-"):
                    continue
                source = args.binaries / (binary + suffix)
                if not source.is_file() or source.stat().st_size == 0:
                    parser.error(f"missing compiled binary: {source}")
                shutil.copy2(source, binary_dir / source.name)
        licenses = root / "licenses"
        licenses.mkdir()
        for name in ("LICENSE", "THIRD_PARTY_NOTICES.md", "Cargo.lock", "native-deps.lock"):
            shutil.copy2(repo / name, licenses / name)
        shutil.copy2(repo / "apps/client/pubspec.lock", licenses / "flutter-pubspec.lock")
        for name in ("LICENSE", "PATENTS", "AUTHORS"):
            shutil.copy2(args.native_source / name, licenses / ("libvpx-" + name))
        for name in ("signing.md", "support-matrix.md"):
            shutil.copy2(repo / "docs" / name, root / name)
        (root / "BUILD-INFO.json").write_text(json.dumps({
            "product": "DeskHarbor", "qualification": "development-unqualified",
            "revision": args.revision, "platform": args.platform,
            "desktop_sign_in_certified": False, "apple_notarized": False,
            "host_service_installed": False,
        }, indent=2) + "\n", encoding="utf-8")
        (root / "README-DEVELOPMENT.txt").write_text(
            "DeskHarbor development build\n\n"
            "This package is not a qualified v1 release. Desktop sign-in hosting,\n"
            "hardware compatibility and performance have not passed release gates.\n"
            "No privileged service is installed or enabled by this archive.\n"
            "Run the desktop application from its complete app directory; keep its\n"
            "libraries and assets together. dh-hostd/dh-cli are development tools.\n"
            "Read support-matrix.md and signing.md before use.\n"
            "macOS signatures are ad-hoc, without Developer ID or notarization.\n"
            "Verify the archive with gh attestation verify FILE --repo GtechGovind/deskharbor.\n",
            encoding="utf-8",
        )
        manifest = "".join(f"{digest(path)}  {path.relative_to(root).as_posix()}\n"
                           for path in sorted(root.rglob("*")) if path.is_file() and not path.is_symlink())
        (root / "SHA256SUMS").write_text(manifest, encoding="utf-8")
        if args.platform == "windows-x64" or args.platform == "android-arm64":
            artifact = output / (stem + ".zip")
            if artifact.exists():
                parser.error(f"refusing to overwrite {artifact}")
            with zipfile.ZipFile(artifact, "x", zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
                for path in sorted(root.rglob("*")):
                    archive.write(path, path.relative_to(root.parent).as_posix())
        else:
            artifact = output / (stem + ".tar.gz")
            if artifact.exists():
                parser.error(f"refusing to overwrite {artifact}")
            with tarfile.open(artifact, "x:gz", dereference=False) as archive:
                archive.add(root, arcname=stem)
        (output / (artifact.name + ".sha256")).write_text(f"{digest(artifact)}  {artifact.name}\n", encoding="utf-8")
        print(artifact)


if __name__ == "__main__":
    main()
