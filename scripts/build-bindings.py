"""Build native artifacts for the current host. Runtime configuration always comes from callers."""

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--release", action="store_true")
parser.add_argument("--node-only", action="store_true", help="Build only the Node binding")
parser.add_argument(
    "--python",
    action="store_true",
    help="Also install the mixed Python package into the active virtualenv",
)
args = parser.parse_args()
if args.node_only and args.python:
    parser.error("--node-only cannot be combined with --python")
command = ["cargo", "build", "-p", "node"]
if not args.node_only:
    command += ["-p", "go"]
if args.release:
    command.append("--release")
subprocess.run(command, cwd=ROOT, check=True)
artifacts = ROOT / "target" / ("release" if args.release else "debug")
name = (
    "node.dll"
    if platform.system() == "Windows"
    else "libnode.dylib"
    if platform.system() == "Darwin"
    else "libnode.so"
)
shutil.copy2(artifacts / name, ROOT / "packages/node/stargate.node")
if args.python:
    subprocess.run([sys.executable, str(ROOT / "scripts/prepare-python-metadata.py")], check=True)
    subprocess.run(
        [sys.executable, "-m", "maturin", "develop"]
        + (["--release"] if args.release else []),
        cwd=ROOT / "packages/python",
        env={**os.environ, "VIRTUAL_ENV": sys.prefix},
        check=True,
    )
if args.node_only:
    print("Node native binding built.")
else:
    print(
        "Native bindings built. Point CGO_LDFLAGS and your system loader at "
        + str(artifacts)
        + " for Go."
    )
