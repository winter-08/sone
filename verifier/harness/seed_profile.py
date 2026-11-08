#!/usr/bin/env python3
"""Seed (or read back) a SONE profile without running SONE.

settings.json is AES-256-GCM: b"SONE" | 0x01 | 12-byte nonce | ciphertext+tag, keyed by the
32 raw bytes in <config>/sone.key whenever no Secret Service answers (true in our private bus).

    seed_profile.py seed  CONFIG_DIR '{"mcp_enabled": true, ...}'
    seed_profile.py dump  CONFIG_DIR
"""
import json
import os
import sys
from pathlib import Path

from cryptography.hazmat.primitives.ciphers.aead import AESGCM


def key(cfg: Path) -> bytes:
    p = cfg / "sone.key"
    if not p.exists():
        cfg.mkdir(parents=True, exist_ok=True)
        p.write_bytes(os.urandom(32))
        p.chmod(0o600)
    return p.read_bytes()


def dump(cfg: Path) -> dict:
    data = (cfg / "settings.json").read_bytes()
    assert data[:5] == b"SONE\x01", "not a SONE v1 blob"
    return json.loads(AESGCM(key(cfg)).decrypt(data[5:17], data[17:], None))


def seed(cfg: Path, overrides: dict) -> None:
    current = dump(cfg) if (cfg / "settings.json").exists() else {}
    current.update(overrides)
    nonce = os.urandom(12)
    blob = b"SONE\x01" + nonce + AESGCM(key(cfg)).encrypt(nonce, json.dumps(current).encode(), None)
    (cfg / "settings.json").write_bytes(blob)


if __name__ == "__main__":
    cmd, cfg = sys.argv[1], Path(sys.argv[2])
    if cmd == "seed":
        seed(cfg, json.loads(sys.argv[3]))
    print(json.dumps(dump(cfg), indent=2))
