# Local accounts and administrators

Choose email/password, OIDC, or both through configuration. Local accounts are part of the same runtime and embedded panel; no additional package, administration service, or bootstrap token is required. Omit `local` or set it to `null` to keep local accounts disabled.

## Enable local accounts

Python:

```python
from stargate import Auth, Local, Turso

auth = Auth(
    base_url="https://app.example.com",
    storage=Turso("./stargate.db"),
    local=Local(),
)
auth.mount(app)
```

Node:

```javascript
const { createAuth, turso } = require('@syntropika/stargate');
const auth = await createAuth({
  baseUrl: 'https://app.example.com',
  storage: turso('./stargate.db'),
  local: {},
});
app.use(auth.middleware());
```

Rust:

```rust
use stargate::{AuthConfig, LocalConfig, Stargate, StorageConfig, TursoConfig};

let mut config = AuthConfig::new(
    "https://app.example.com".parse()?,
    StorageConfig::Turso(TursoConfig::new("./stargate.db")),
);
config.local = Some(LocalConfig::default());
let auth = Stargate::new(config).await?;
let app = auth.mount(app);
```

Go:

```go
auth, err := stargate.New(stargate.Config{
    BaseURL: "https://app.example.com",
    Storage: stargate.Turso("./stargate.db"),
    Local: &stargate.LocalConfig{},
})
if err != nil { return err }
defer auth.Close()
mux.Handle("/auth/", auth.Handler())
```

Keep the host-language integration from the [integration guide](integrations.md). To offer both methods, also supply the usual `oidc` providers (`OIDC` in Go). Local configuration uses `login_attempts`, `login_window_seconds`, and `initial_admin_user_id` in JSON, Node and Python; Go exposes the corresponding exported fields. An empty local configuration uses the defaults.

## First-run setup

On a fresh database, open `/auth/`. The page asks for an email and a password of at least 12 characters, with confirmation. The first signup transaction to commit creates the administrator and its browser session atomically. Simultaneous requests cannot create multiple initial administrators. Public signup then closes permanently, including after restarts.

Complete setup before exposing a fresh installation to other people: whoever completes it first becomes its administrator. OIDC sign-in is held until setup is complete when both methods are enabled on a fresh database.

Additional local users are created by an administrator in `/auth/users`. Public self-registration after setup is not supported. Administrators choose an initial password and share it through a private channel. Users can change that password in their profile.

## Existing OIDC databases

Enabling local accounts on a database that already contains users requires an explicit `local.initial_admin_user_id` on its first initialization. Use the exact ID of an existing active user; this is an identifier, not a secret or environment token. That user keeps OIDC sign-in and gains administrator access. No local password is added to their account automatically.

For example, Python accepts `local=Local(initial_admin_user_id=existing_user_id)`. The ID is available in the authenticated user's `/auth/api/me` identity. Initialization is recorded permanently. You can remove this setting afterwards; leaving it configured does not restore a role later removed by an administrator. Startup fails if initial administration cannot be established.

Local accounts and OIDC identities remain distinct even when their emails match. OIDC uses exact issuer and subject; it never grants administration based on email or an external role claim.

## Manage users and access

The Users view appears for active administrators when local accounts are enabled. It lists local and OIDC users, supports creating local accounts, assigning `user` or `administrator`, disabling or enabling accounts, and revoking all sessions and API keys. The list loads up to 100 users at a time.

At least one administrator must remain active. The database rejects attempts to disable or demote the last administrator, including concurrent changes. Disabling an account also revokes its current credentials. Re-enabling permits a new sign-in and never restores old credentials. Revoking access alone leaves the account enabled, so it can sign in again.

Administration requires a browser session and a fresh stored role check. API keys cannot administer accounts, regardless of their scopes. The normalized identity includes `claims.stargate.role` for presentation; authorization does not trust client-supplied role claims. Administrator status does not implicitly grant application scopes. `session.scopes` remains the host's explicit grant for all sessions.

## Password and request protection

Passwords use Argon2id with a random salt, 19 MiB of memory, two iterations and one lane. Passwords require at least 12 Unicode characters and at most 1024 UTF-8 bytes. They are not trimmed. Local emails are ASCII, trimmed and lowercased, up to 254 bytes. An email is an account identifier; this release does not verify ownership.

Password verification and hashing run outside async workers with at most four concurrent jobs per runtime. Login and password-change attempts are limited in memory: by default, 10 attempts per account and 100 per peer in a 300-second window. Successful attempts also count. State is bounded and resets when the runtime restarts; deployments with multiple runtimes need a shared limit at their ingress. Hosts must supply the real peer address for independent peer limits; requests without it share a single peer bucket.

Setup and login require an exact configured `Origin`, a separate secure HttpOnly CSRF cookie and the `X-Stargate-CSRF` value from `/auth/api/local`. Authenticated changes use the session CSRF value from `/auth/api/me`. The panel supplies both automatically. A password change verifies the current password, updates the hash and replaces the browser session in one transaction. All previous sessions are revoked; API keys remain until explicitly revoked. An in-progress login cannot create a session using a superseded hash.

## HTTP endpoints

All routes use the configured prefix; `/auth` is the default.

| Endpoint | Access and behavior |
| --- | --- |
| `GET /auth/api/local` | Public enabled/setup status and anonymous CSRF token |
| `POST /auth/api/local/setup` | First-run administrator creation; returns 201 and a session cookie |
| `POST /auth/api/local/login` | Email/password sign-in; returns 200 and a session cookie |
| `POST /auth/api/local/password` | Session, current password and new password; returns a replacement cookie |
| `GET /auth/api/users?after={id}` | Administrator; `{users, next_cursor}` without credentials |
| `POST /auth/api/users` | Administrator; `{email, password, role?}`; default role is `user` |
| `PATCH /auth/api/users/{id}` | Administrator; `{role, disabled}` |
| `DELETE /auth/api/users/{id}/access` | Administrator; revokes all sessions and keys |

Setup/login accept `{email, password}`. Password changes accept `{current_password, new_password}`. Requests use JSON. Invalid credentials return 401, CSRF or access failures 403, setup/duplicate email/last-administrator conflicts 409, and limited requests 429 with `Retry-After`. Storage failures return 503. A committed account or password change remains committed if subsequent audit persistence fails; the user can sign in using the committed password.

Email verification, email delivery, invitations, forgotten-password recovery, MFA and public signup after setup are future work. This delivery does not configure an SMTP service.
