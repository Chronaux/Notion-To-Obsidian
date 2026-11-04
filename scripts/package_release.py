#!/usr/bin/env python3
"""Package the cross-compiled Windows EXE into a portable release ZIP."""
from __future__ import annotations

import hashlib
import pathlib
import shutil
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


def main() -> None:
    if not EXE_SOURCE.is_file():
        raise SystemExit(f"Windows executable was not built: {EXE_SOURCE}")
    image = EXE_SOURCE.read_bytes()
    if image[:2] != b"MZ":
        raise SystemExit("Not a Windows PE executable (MZ header missing)")
    pe = int.from_bytes(image[0x3c:0x40], "little")
    if image[pe:pe + 4] != b"PE\x00\x00" or int.from_bytes(image[pe + 4:pe + 6], "little") != 0x8664:
        raise SystemExit("Windows PE x64 machine type missing")

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
    with zipfile.ZipFile(ARCHIVE, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for item in sorted(WORK.rglob("*")):
            if item.is_file():
                archive.write(item, item.relative_to(WORK).as_posix())

    with zipfile.ZipFile(ARCHIVE) as archive:
        assert archive.testzip() is None, "Invalid release ZIP"
        assert EXE_NAME in archive.namelist(), "EXE is missing from archive"
    (OUT / "SHA256SUMS.txt").write_text(
        f"{sha256(ARCHIVE)}  {ARCHIVE.name}\n", encoding="utf-8"
    )
    shutil.rmtree(WORK)
    print(f"Release ready: {ARCHIVE.name}, {ARCHIVE.stat().st_size} bytes, version {VERSION}")


if __name__ == "__main__":
    main()
