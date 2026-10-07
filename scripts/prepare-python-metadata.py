"""Materialize canonical package metadata when Git cannot create symbolic links."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
for name in ("LICENSE", "README.md"):
    target = ROOT / "packages/python" / name
    if not target.is_symlink():
        target.write_text((ROOT / name).read_text())
