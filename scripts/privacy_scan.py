#!/usr/bin/env python3
"""Detect likely private information in tracked first-party text files."""
from __future__ import annotations

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
IGNORE = {"THIRD_PARTY_NOTICES.txt", "scripts/privacy_scan.py"}
SKIP_PREFIX = ("licenses/",)
SKIP_SUFFIX = (".ico", ".exe", ".zip", ".pdb", ".lock")
RULES = {
    "credentials": re.compile(r"(?:ghp_|gho_|github_pat_|ntn_|sk-[A-Za-z0-9]{16}|AKIA[A-Z0-9]{16}|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----)"),
    "personal_email": re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b"),
    "windows_home": re.compile(r"\b[A-Za-z]:[/\\]Users[/\\][^/\\\s\"']+"),
    "unix_home": re.compile(r"/(?:Users|home)/[^/\s\"']+"),
    "internal_host": re.compile(r"https?://(?:[a-z0-9-]+\.)*(?:intranet|internal|helpdesk|service-desk|itsm)\.[a-z0-9.-]+", re.I),
    "notion_private_link": re.compile(r"https?://(?:www\.)?(?:notion\.so|notion\.site|app\.notion\.com)/[^\s\"'<>)]*[a-f0-9]{32}", re.I),
}

def main() -> int:
    try:
        paths = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
    except (OSError, subprocess.CalledProcessError):
        print("Run this scanner inside a Git checkout.", file=sys.stderr)
        return 2
    findings = []
    for path in paths:
        if not path or path in IGNORE or path.startswith(SKIP_PREFIX) or path.endswith(SKIP_SUFFIX):
            continue
        file = ROOT / path
        if not file.is_file():
            continue
        try:
            data = file.read_text(encoding="utf-8")
        except (UnicodeError, OSError):
            continue
        for line_number, line in enumerate(data.splitlines(), 1):
            for label, pattern in RULES.items():
                if pattern.search(line):
                    findings.append((path, line_number, label))
    if findings:
        for path, line, reason in findings:
            print(f"{path}:{line}: possible {reason} (value hidden)")
        print(f"{len(findings)} finding(s); inspect manually.")
        return 1
    print("No high-confidence private patterns found in tracked first-party text.")
    print("This is not a history, binary, screenshot or manual content audit.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
