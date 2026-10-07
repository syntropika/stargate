"""Install staged npm tarballs and run the Express contract outside the checkout."""

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NPM = shutil.which("npm")
if NPM is None:
    raise SystemExit("npm is required")
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--packages", type=Path, required=True)
args = parser.parse_args()
packages = args.packages.resolve()
native = [p for p in packages.iterdir() if p.is_dir() and p.name != "main"]
if len(native) != 1:
    raise SystemExit("Provide one platform package alongside the main package")
with tempfile.TemporaryDirectory(prefix="stargate-npm-install-") as directory:
    sandbox = Path(directory)
    tarballs = []
    for package in [native[0], packages / "main"]:
        packed = subprocess.check_output(
            [NPM, "pack", str(package), "--pack-destination", directory, "--json"],
            text=True,
        )
        tarballs.append(str(sandbox / json.loads(packed)[0]["filename"]))
    (sandbox / "package.json").write_text('{"private": true}\n')
    subprocess.run(
        [NPM, "install", "--ignore-scripts", "--no-audit", "--no-fund", *tarballs, "express"],
        cwd=sandbox,
        check=True,
    )
    tests = (ROOT / "packages/node/test.js").read_text()
    tests = tests.replace("require('./index')", "require('@syntropika/stargate')")
    (sandbox / "test.js").write_text(tests)
    subprocess.run(["node", "--test", "test.js"], cwd=sandbox, check=True)
