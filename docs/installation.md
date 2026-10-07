# Installation

Install Stargate for the language hosting your HTTP server. Each integration
uses the same Rust authentication runtime and embedded account panel.

| Host | Package | Import |
| --- | --- | --- |
| Node / Express | `@syntropika/stargate` on npm | `require('@syntropika/stargate')` |
| Python / FastAPI or Starlette | `syntropika-stargate` on PyPI | `from stargate import Auth` |
| Rust / Axum | `syntropika-stargate` on crates.io | `use stargate::Stargate` with a dependency alias |
| Go / net/http | Repository source with the Rust C ABI | `github.com/syntropika/stargate/packages/go` |

```sh
# Node / Express
npm install @syntropika/stargate express

# Python / FastAPI
python -m pip install syntropika-stargate fastapi

# Rust / Axum
cargo add syntropika-stargate --rename stargate
```

The npm package selects an optional native binary for your operating system and
architecture. Python installs a native wheel when one is available. The account
panel is compiled into the runtime; consumers do not need a frontend build or
a separate Node.js server to serve it.

## Supported platforms

| Platform | Node prebuilt binary | Python prebuilt wheel |
| --- | --- | --- |
| Linux x64 | glibc 2.35 or later | manylinux 2.28 |
| macOS arm64 | Available | Available |
| Windows x64 | Available | Available |

Node requires version 20 or later. Python wheels use the CPython 3.10+ stable
ABI. Other platforms require a source build. Rust applications compile the
runtime from source; Go links to the Rust C ABI through cgo.

Source builds require Rust 1.90+ and a C/C++ build toolchain. Native bindings
also require Python 3.10+ and the tools for the host language: Node.js 20+ or
Go 1.22+ with cgo. Linux builds need libclang development libraries.

Follow the [integration guide](integrations.md) to build from source, mount
Stargate and protect application routes. Read [configuration and security](configuration.md)
before deploying your application.
