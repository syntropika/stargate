# Website deployment

The public website is a static Astro application in `website/`. It presents
Stargate and renders the repository's Markdown guides under `/docs/`. The guides
in this directory are the source of truth; edit them rather than duplicating
content in the website. Relative guide links are rewritten during the site build.

The landing page includes language-specific installation commands, a keyboard
accessible language selector and copy controls. Go links to its source-build
guide. The site generates a sitemap and robots file from the same documentation
collection. All fonts and browser assets are served from the website's origin.
The EB Garamond and JetBrains Mono font license notices ship under `/licenses/`.

## Local development

Use Node.js 24:

```sh
cd website
npm ci
npm run dev
```

Run `npm run check`, `npm run format:check` and `npm run build` before deploying.
The build outputs static HTML, CSS, JavaScript and fonts in `website/dist/`.
The runtime authentication package and its embedded account panel remain separate
from this public documentation website.

## Cloudflare Workers

Wrangler serves the generated files as Worker static assets. The website's
custom domain is configured in `website/wrangler.jsonc`. A missing URL returns
an actual 404; documentation paths are not rewritten to the homepage.

For local deployment, provide `CLOUDFLARE_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID` through the process environment, then run:

```sh
npm run build
npm run deploy
```

## GitHub Actions

The `website` workflow validates changes on pull requests. On pushes to `main`,
it deploys after validation; it can also run manually from `main`. Changes to
website source, documentation or branding trigger the workflow. Package release
publishing runs independently.

Configure these repository values in GitHub Settings → Secrets and variables → Actions:

| Kind | Name | Purpose |
| --- | --- | --- |
| Secret | `CLOUDFLARE_API_TOKEN` | A Cloudflare API token allowed to deploy Workers and manage the custom domain |
| Variable | `CLOUDFLARE_ACCOUNT_ID` | The account that owns the Worker and domain zone |

Use a token scoped to the deployment account and the domain zone, with Account
Workers Scripts Edit, Account Workers Routes Edit and Zone Read permissions as
required by Wrangler's custom-domain deployment. Consult Cloudflare's current
Workers deployment token template when creating it. Do not store tokens in the
repository or put them in client-exposed build variables.

The workflow deploys only `main`. It does not merge pull requests, publish Rust,
Node or Python packages, or deploy the authentication runtime.
