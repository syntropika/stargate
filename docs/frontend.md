# Account frontend

The account panel is a private React application in `crates/ui/frontend`. Vite
builds React and Tailwind into `crates/ui/assets/app.js` and `app.css`. The Rust
`ui` crate embeds those files at compile time; every native binding ships the
same panel. There is no separately published frontend package or runtime Node.js
dependency.

## Build

Use Node.js 24 and npm:

```sh
cd crates/ui/frontend
npm ci
npm run lint
npm run format:check
npm run build
```

Commit source changes and the regenerated assets together. `scripts/check.sh`
checks Oxlint, Oxfmt and TypeScript and rebuilds the frontend before validating the Rust runtime
and bindings. CI also rejects changes that leave the checked-in assets stale.
Rebuild the host's Rust binary or native binding after changing embedded assets.
Registry consumers use the compiled assets without installing frontend tooling.
Use `npm run format` to apply Oxfmt to the frontend source. Oxlint includes React,
TypeScript and JSX accessibility rules; warnings fail the check. Generated assets
are produced by Vite and are outside the formatter and linter targets.

## Preview and integration

`npm run dev` starts Vite for layout development. Its `index.html` supplies a
development configuration; it does not provide authentication or a mock API.
Use a host application running Stargate to exercise real sign-in, keys and
sessions after rebuilding the assets.

Rust serves an HTML shell with public configuration (path prefix, app name, logo,
provider names and current route). React renders the panel in the browser and
uses the existing same-origin account API. No credentials are embedded in that
configuration. This is client rendering; the server does not run React or a
JavaScript engine. Scripts and styles load from the configured Stargate prefix
under the existing Content Security Policy.

## Layout and branding

Authenticated views share horizontal navigation on desktop and mobile. Profile, API keys and sessions remain distinct views. The key inventory precedes the creation form; the one-time secret appears inline
after creation. Mobile registers reflow into labeled records. Lists show explicit
loading, error and empty states. Unauthenticated visitors see configured sign-in
providers without account navigation.

`panel.css` imports Tailwind and defines scoped panel components and semantic
color/spacing variables. Preserve `--accent` and the supported element IDs when
changing components. Host stylesheets load after the embedded CSS and can
override the panel. See [configuration](configuration.md) for branding settings.

The default Stargate portal-and-star logo is served under the configured prefix
at `/assets/stargate.svg` and is used as the favicon unless the host provides its
own logo. EB Garamond and JetBrains Mono Latin variable fonts are embedded in the
CSS; the panel does not contact a font service. The Content Security Policy
allows `data:` only for fonts, alongside same-origin font loading.

React, React DOM, Tailwind and font notices are retained in the generated bundle
and `assets/THIRD_PARTY_LICENSES.txt`. Stargate's own code remains Apache 2.0;
the embedded fonts use the SIL Open Font License.
