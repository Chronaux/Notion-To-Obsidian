#!/usr/bin/env python3
"""Build a portable Windows release ZIP from a validated, unwrapped file bundle."""
from __future__ import annotations

import hashlib
import pathlib
import shutil
import sys
import tomllib
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
EXE_SOURCE = ROOT / "target/x86_64-pc-windows-gnu/release/notion-to-obsidian.exe"
EXE_NAME = f"NotionToObsidian_v{VERSION}.exe"
OUT = ROOT / "dist"
WORK = OUT / "bundle"
ARCHIVE = OUT / "NotionToObsidian_Windows_x64.zip"


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_exe(path: pathlib.Path) -> None:
    if not path.is_file():
        raise SystemExit(f"Windows executable was not built: {path}")
    with path.open("rb") as stream:
        header = stream.read(256)
        if header[:2] != b"MZ":
            raise SystemExit("Not a Windows PE executable (MZ header missing)")
        offset = int.from_bytes(header[0x3c:0x40], "little")
        stream.seek(offset)
        pe = stream.read(6)
        if pe[:4] != b"PE\x00\x00" or int.from_bytes(pe[4:6], "little") != 0x8664:
            raise SystemExit("Windows PE x64 machine type missing")


def repack() -> None:
    # Both release and smoke jobs download unpacked files as a GitHub artifact.
    # GitHub itself wraps those files into a download ZIP; uploading another
    # ZIP into it would force users to extract twice.
    if not WORK.is_dir():
        raise SystemExit(f"Missing unwrapped Windows bundle: {WORK}")
    exe = WORK / EXE_NAME
    verify_exe(exe)
    checksum_path = WORK / "SHA256SUMS.txt"
    expected = checksum_path.read_text(encoding="utf-8").split()[0]
    if sha256(exe) != expected:
        raise SystemExit("EXE SHA-256 mismatch in Windows artifact")

    with zipfile.ZipFile(ARCHIVE, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for item in sorted(WORK.rglob("*")):
            if item.is_file():
                archive.write(item, item.relative_to(WORK).as_posix())

    with zipfile.ZipFile(ARCHIVE) as archive:
        assert archive.testzip() is None, "Invalid release ZIP"
        assert EXE_NAME in archive.namelist(), "EXE is missing from archive"
        assert not any(p.endswith(".zip") for p in archive.namelist()), "Nested archive detected"
    (OUT / "SHA256SUMS.txt").write_text(
        f"{sha256(ARCHIVE)}  {ARCHIVE.name}\n", encoding="utf-8"
    )
    print(f"Release ready: {ARCHIVE.name}, {ARCHIVE.stat().st_size} bytes, version {VERSION}")


def main() -> None:
    if len(sys.argv) > 1:
        if sys.argv[1:] == ["--repack"]:
            repack()
            return
        raise SystemExit("Usage: package_release.py [--repack]")

    verify_exe(EXE_SOURCE)
    if OUT.exists():
        shutil.rmtree(OUT)
    WORK.mkdir(parents=True)
    shutil.copy2(EXE_SOURCE, WORK / EXE_NAME)

    for item in ["README.md", "CHANGELOG.md", "THIRD_PARTY_NOTICES.txt", "docs/USAGE.md"]:
        target = WORK / item
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / item, target)
    shutil.copytree(ROOT / "licenses", WORK / "licenses")

    (WORK / "SHA256SUMS.txt").write_text(
        f"{sha256(WORK / EXE_NAME)}  {EXE_NAME}\n", encoding="utf-8"
    )
    # Leave WORK intact for upload-artifact: its generated ZIP already serves
    # as the sole outer archive when downloaded from GitHub Actions.
    repack()


if __name__ == "__main__":
    main()
