"""Stage publishable Rust and npm packages without renaming the source workspace."""

import argparse
import json
import platform
import re
import shutil
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORKSPACE = tomllib.loads((ROOT / "Cargo.toml").read_text())
VERSION = WORKSPACE["workspace"]["package"]["version"]
REPOSITORY = "https://github.com/syntropika/stargate"
CRATES = {
    "storage": "syntropika-stargate-storage",
    "ui": "syntropika-stargate-ui",
    "core": "syntropika-stargate-core",
    "http": "syntropika-stargate-http",
    "rust": "syntropika-stargate",
}
DESCRIPTIONS = {
    "storage": "Storage contracts and embedded Turso adapter for Stargate",
    "ui": "Embedded account UI assets for Stargate",
    "core": "OIDC, sessions, API keys and authorization for Stargate",
    "http": "HTTP routing and security for the Stargate authentication runtime",
    "rust": "Embedded authentication for Axum: OIDC, sessions, API keys and account UI",
}


def stage_rust(output):
    destination = output / "rust"
    destination.mkdir()
    manifest = (ROOT / "Cargo.toml").read_text()
    manifest = manifest.replace('default-members = ["crates/rust"]', '')
    manifest = manifest.replace('license = "Apache-2.0"', 'license = "Apache-2.0"\nrepository = "' + REPOSITORY + '"')
    for key, dependency in WORKSPACE["workspace"]["dependencies"].items():
        if isinstance(dependency, dict) and "path" in dependency:
            original = dependency.get("package", key)
            manifest = re.sub(
                r"^" + re.escape(key) + r" = .*?$",
                key + ' = { package = "' + CRATES[original] + '", version = "=' + VERSION + '", path = "crates/' + original + '" }',
                manifest,
                flags=re.MULTILINE,
            )
    (destination / "Cargo.toml").write_text(manifest)
    shutil.copy2(ROOT / "Cargo.lock", destination / "Cargo.lock")
    for original, published in CRATES.items():
        source = ROOT / "crates" / original
        target = destination / "crates" / original
        shutil.copytree(source / "src", target / "src")
        if (source / "assets").exists():
            shutil.copytree(source / "assets", target / "assets")
        text = (source / "Cargo.toml").read_text()
        text = text.replace('name = "' + original + '"', 'name = "' + published + '"', 1)
        text = text.replace('license.workspace = true', 'license.workspace = true\nrepository.workspace = true\nreadme = "README.md"\ndescription = "' + DESCRIPTIONS[original] + '"\nautotests = false\nautoexamples = false\ninclude = ["src/**", "assets/**", "Cargo.toml", "README.md", "LICENSE"]')
        if "[dev-dependencies]" in text:
            text = text[:text.index("[dev-dependencies]")]
        if original == "rust":
            text += '\n[lib]\nname = "stargate"\n'
        (target / "Cargo.toml").write_text(text)
        (target / "README.md").write_text('# ' + published + '\n\n' + DESCRIPTIONS[original] + '.\n\nSee [' + REPOSITORY + '](' + REPOSITORY + ') for integration guides and source.\n\nLicensed under Apache 2.0.\n')
        shutil.copy2(ROOT / "LICENSE", target / "LICENSE")
    return destination


def stage_node(output):
    system, arch = platform.system(), platform.machine().lower()
    if system == "Linux" and arch in ("x86_64", "amd64") and platform.libc_ver()[0] == "glibc":
        target, npm_os, npm_cpu, libc = "linux-x64-gnu", "linux", "x64", "glibc"
    elif system == "Darwin" and arch in ("arm64", "aarch64"):
        target, npm_os, npm_cpu, libc = "darwin-arm64", "darwin", "arm64", None
    else:
        raise SystemExit("No prebuilt npm target configured for this platform")
    source = ROOT / "packages/node"
    binary = source / "stargate.node"
    if not binary.exists():
        raise SystemExit("Build the Node native binding first with scripts/build-bindings.py")
    main, native = output / "node/main", output / "node" / target
    main.mkdir(parents=True)
    native.mkdir()
    package = json.loads((source / "package.json").read_text())
    package.pop("devDependencies", None)
    package.pop("scripts", None)
    package["files"] = ["index.js", "index.d.ts", "native.js", "README.md", "LICENSE"]
    for name in ("index.js", "index.d.ts", "native.js"):
        shutil.copy2(source / name, main / name)
    (main / "package.json").write_text(json.dumps(package, indent=2) + "\n")
    native_package = {
        "name": "@syntropika/stargate-" + target,
        "version": VERSION,
        "description": "Native Stargate binary for " + target,
        "license": "Apache-2.0",
        "repository": package["repository"],
        "main": "stargate.node",
        "files": ["stargate.node", "README.md", "LICENSE"],
        "os": [npm_os],
        "cpu": [npm_cpu],
        "engines": package["engines"],
        "publishConfig": {"access": "public"},
    }
    if libc:
        native_package["libc"] = [libc]
    (native / "package.json").write_text(json.dumps(native_package, indent=2) + "\n")
    shutil.copy2(binary, native / binary.name)
    for folder in (main, native):
        shutil.copy2(ROOT / "LICENSE", folder / "LICENSE")
        (folder / "README.md").write_text('# Stargate\n\nEmbedded authentication for Express.\n\n```sh\nnpm install @syntropika/stargate\n```\n\nSee [the integration guide](' + REPOSITORY + '/blob/main/docs/integrations.md#node--express) for configuration and usage.\n\nPrebuilt binaries support Linux x64 with glibc and macOS arm64. Other platforms require a source build.\n\nLicensed under Apache 2.0.\n')
    return main, native


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--only", choices=("rust", "node", "all"), default="all")
    args = parser.parse_args()
    output = args.output.resolve()
    if output == ROOT or ROOT in output.parents:
        raise SystemExit("Stage release packages outside the repository")
    output.mkdir(parents=True, exist_ok=True)
    if args.only in ("rust", "all"):
        print(stage_rust(output))
    if args.only in ("node", "all"):
        for directory in stage_node(output):
            print(directory)


if __name__ == "__main__":
    main()
