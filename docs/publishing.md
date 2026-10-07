# Publishing Stargate

Release builds run through [release.yml](../.github/workflows/release.yml). The workflow builds and tests packages first; publishing is an explicit input and requires a matching version tag.

## Package names

| Registry | Public package | Import |
| --- | --- | --- |
| npm | `@syntropika/stargate` | `require('@syntropika/stargate')` |
| PyPI | `syntropika-stargate` | `from stargate import Auth` |
| crates.io | `syntropika-stargate` | `use stargate::Stargate` with a dependency alias |

For Rust applications:

```toml
[dependencies]
stargate = { package = "syntropika-stargate", version = "0.1" }
```

The source workspace keeps its unprefixed package names. `scripts/prepare-release.py` stages a separate publishable workspace with registry names and versioned dependencies. The Rust integration depends on four supporting published crates: `syntropika-stargate-storage`, `syntropika-stargate-ui`, `syntropika-stargate-core` and `syntropika-stargate-http`. Storage adapters remain inside the storage crate.

npm uses two optional native packages: `@syntropika/stargate-linux-x64-gnu` and `@syntropika/stargate-darwin-arm64`. The release workflow builds Linux x64 on Ubuntu 22.04 and macOS arm64. Linux Node binaries require glibc 2.35 or later. Python wheels use the CPython 3.10+ stable ABI, with manylinux 2.28 x64 and macOS arm64 builds. Other platforms require a source build. Registry publication is independent of the Git repository's visibility.

## GitHub environments and secrets

Create GitHub environments named `npm`, `pypi` and `crates-io` under the repository's **Settings → Environments**. No repository variables are required.

The first publication requires these temporary environment secrets:

| Environment | Secret | Value |
| --- | --- | --- |
| `npm` | `NPM_TOKEN` | A granular npm access token authorized to create and publish the three public packages in the `syntropika` npm organization, with permission to publish from CI without an interactive 2FA challenge |
| `crates-io` | `CARGO_REGISTRY_TOKEN` | A crates.io API token authorized to create and publish the five Stargate crates |
| `pypi` | None | Configure a pending Trusted Publisher before the first release |

The `syntropika` organization must exist on npm; membership in a GitHub organization does not create an npm organization. The publishing account needs write access to that npm scope. The crates.io account needs a verified email address.

Select `bootstrap=true` only for the first publication. npm and crates.io require existing packages before configuring Trusted Publishing. Once it is configured for every package, remove both temporary secrets and use `bootstrap=false`. `CARGO_REGISTRY_TOKEN` is then set automatically to a short-lived token by the authentication action. Do not create `NODE_AUTH_TOKEN`, `PYPI_API_TOKEN`, `TWINE_PASSWORD` or OIDC request variables for the normal workflow. GitHub supplies the OIDC variables through `id-token: write`.

## Trusted Publisher configuration

Use these exact values in each registry's GitHub publisher configuration:

| Field | Value |
| --- | --- |
| GitHub owner / organization | `syntropika` |
| Repository | `stargate` |
| Workflow filename | `release.yml` |
| GitHub environment | `npm`, `pypi` or `crates-io`, matching the registry |

Enter only `release.yml`, not its full path.

### PyPI

Open [PyPI Publishing](https://pypi.org/manage/account/publishing/) and add a **pending publisher** for `syntropika-stargate`, with owner `syntropika`, repository `stargate`, workflow `release.yml` and environment `pypi`. This authorizes creation of the project on its first OIDC publication. No PyPI token is needed.

### npm

After the bootstrap publication, open each of the three npm packages and add a GitHub Trusted Publisher under **Settings → Trusted publishing**. Use the shared fields above and environment `npm`; allow direct `npm publish`. Complete the next successful publication within the registry's configuration validity window. The workflow installs npm 11, which supports OIDC publishing.

### crates.io

After the bootstrap publication, open **Settings → Trusted Publishing** for each of the five Rust crates. Add the GitHub configuration with environment `crates-io`. The workflow uses `rust-lang/crates-io-auth-action` to obtain and revoke a temporary token.

## Build and release

1. Push the repository and its workflows to GitHub.
2. Configure the environments, bootstrap secrets and pending PyPI publisher.
3. Run **Actions → release → Run workflow** with `publish=false` to review build artifacts.
4. Keep `Cargo.toml`, `packages/node/package.json` and `packages/python/pyproject.toml` versions aligned, including the npm optional native dependency versions.
5. Create and push the matching tag, such as `v0.1.0`.
6. Run the release workflow on that tag with `publish=true` and `bootstrap=true` for the first release. Future releases use `bootstrap=false`.
7. Configure Trusted Publishers for npm and crates.io, then remove the bootstrap secrets.

Publishing is not atomic across registries. If a job fails after uploading some packages, inspect each registry before retrying; registry versions are immutable and already uploaded versions cannot be overwritten. Fix a partial release deliberately rather than assuming the complete workflow can be rerun unchanged.

Local staging does not upload anything:

```sh
python3 scripts/build-bindings.py --release
python3 scripts/prepare-release.py --output /absolute/path/outside/the/checkout
```

Build Python distributions from `packages/python` so maturin reads its `pyproject.toml`. Passing only the Rust manifest from the repository root produces a different package and omits the mixed Python wrapper.

## Registry documentation

- [npm Trusted Publishing](https://docs.npmjs.com/trusted-publishers/)
- [npm trust prerequisites](https://docs.npmjs.com/cli/v11/commands/npm-trust)
- [PyPI Trusted Publishing](https://docs.pypi.org/trusted-publishers/)
- [crates.io Trusted Publishing](https://crates.io/docs/trusted-publishing)
