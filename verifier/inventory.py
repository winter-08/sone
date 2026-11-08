#!/usr/bin/env python3
"""Extract SONE's feature surface from source into a machine-readable inventory.

The inventory is the denominator of the Rune port: every item in it must be
covered by at least one passing verifier scenario (or an explicit waiver)
before the port counts as done. Re-run it on each upstream release to catch
new surface the port hasn't picked up yet.

Usage:
    python3 -I verifier/inventory.py [REPO_ROOT] [--json OUT] [--diff OLD.json]

Stdlib only, and it never imports or executes repo code: it reads files as text.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


def read_all(root: Path, rel: str, suffixes: tuple[str, ...], skip_tests: bool = False) -> list[tuple[Path, str]]:
    out = []
    base = root / rel
    if not base.exists():
        return out
    for p in sorted(base.rglob("*")):
        if not p.is_file() or p.suffix not in suffixes:
            continue
        if skip_tests and (".test." in p.name or p.name == "tests.rs"):
            continue
        out.append((p, p.read_text(encoding="utf-8", errors="replace")))
    return out


def uniq(xs):
    return sorted(set(xs))


def tauri_commands(rust: list[tuple[Path, str]]) -> list[str]:
    """Names registered in tauri::generate_handler![...] (the real IPC surface)."""
    for _, text in rust:
        m = re.search(r"generate_handler!\[(.*?)\]\s*\)", text, re.S)
        if m:
            body = re.sub(r"//[^\n]*", "", m.group(1))
            return uniq(s.strip().split("::")[-1] for s in body.split(",") if s.strip())
    return []


def section(root: Path) -> dict:
    rust = read_all(root, "src-tauri/src", (".rs",))
    rust_src = [(p, t) for p, t in rust if p.name != "tests.rs"]
    ts_all = read_all(root, "src", (".ts", ".tsx"))
    ts = [(p, t) for p, t in ts_all if ".test." not in p.name]

    def grab(files, pattern, group=1):
        return uniq(m.group(group) for _, t in files for m in re.finditer(pattern, t))

    types_ts = (root / "src/types.ts").read_text(encoding="utf-8") if (root / "src/types.ts").exists() else ""
    view_union = re.search(r"type ViewTarget\s*=(.*?);\s*\n\s*\n", types_ts, re.S)
    views = uniq(re.findall(r'type:\s*"([a-zA-Z]+)"', view_union.group(1))) if view_union else []

    settings_fields = []
    for _, t in rust_src:
        m = re.search(r"pub struct Settings\s*\{(.*?)\n\}", t, re.S)
        if m:
            settings_fields = re.findall(r"pub ([a-z_0-9]+)\s*:", m.group(1))
            break

    vitest = {}
    for p, t in ts_all:
        if ".test." in p.name:
            vitest[str(p.relative_to(root))] = re.findall(r'^\s*(?:it|test)\(\s*["\'`](.+?)["\'`]', t, re.M)

    return {
        "tauri_commands": tauri_commands(rust_src),
        "frontend_invokes": grab(ts, r'invoke(?:<[^>]*>)?\(\s*"([a-z0-9_]+)"'),
        "events_emitted_rust": grab(rust_src, r'\.emit(?:_to)?\(\s*(?:"[a-z]+",\s*)?"([a-z0-9:_-]+)"'),
        "events_listened_frontend": grab(ts, r'listen(?:<[^>]*>)?\(\s*"([a-z0-9:_-]+)"'),
        "views": views,
        "shortcuts": grab([(p, t) for p, t in ts if p.name == "shortcuts.ts"], r'id:\s*"([a-zA-Z.-]+)"'),
        "persisted_frontend_keys": grab(ts, r'(?:atomWithStorage|localStorage\.(?:get|set)Item)\(\s*"([^"]+)"'),
        "settings_json_fields": settings_fields,
        "mcp_tools": grab([(p, t) for p, t in rust_src if "/mcp/tools/" in str(p)],
                          r'tool\(\s*name\s*=\s*"([a-z_]+)"'),
        "external_hosts": grab(rust_src, r'"https?://([a-z0-9.-]+\.[a-z]{2,})'),
        "vitest_cases": vitest,
        "cargo_test_count": sum(len(re.findall(r"#\[(?:tokio::)?test", t)) for _, t in rust),
    }


def summarize(inv: dict) -> str:
    lines = []
    for k, v in inv.items():
        if k == "vitest_cases":
            lines.append(f"{k:28} {sum(len(x) for x in v.values())} cases in {len(v)} files")
        elif isinstance(v, list):
            lines.append(f"{k:28} {len(v)}")
        else:
            lines.append(f"{k:28} {v}")
    unused = sorted(set(inv["tauri_commands"]) - set(inv["frontend_invokes"]))
    lines.append(f"\ncommands never invoked from the frontend ({len(unused)}): {', '.join(unused) or '-'}")
    return "\n".join(lines)


def diff(old: dict, new: dict) -> str:
    out = []
    for k in new:
        if isinstance(new[k], list):
            added, removed = sorted(set(new[k]) - set(old.get(k, []))), sorted(set(old.get(k, [])) - set(new[k]))
            if added or removed:
                out.append(f"{k}: +{added} -{removed}")
    return "\n".join(out) or "no surface changes"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("root", nargs="?", default=str(Path(__file__).resolve().parent.parent))
    ap.add_argument("--json", help="write the inventory here")
    ap.add_argument("--diff", help="compare against an earlier inventory JSON")
    a = ap.parse_args()
    inv = section(Path(a.root))
    if a.json:
        Path(a.json).write_text(json.dumps(inv, indent=2) + "\n", encoding="utf-8")
    print(summarize(inv))
    if a.diff:
        print("\n" + diff(json.loads(Path(a.diff).read_text(encoding="utf-8")), inv))
    return 0


if __name__ == "__main__":
    sys.exit(main())
