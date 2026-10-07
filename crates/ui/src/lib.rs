//! Production assets are compiled into the Rust artifact.
pub const JAVASCRIPT: &str = include_str!("../assets/app.js");
pub const STYLESHEET: &str = include_str!("../assets/app.css");
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn page(prefix: &str, name: &str, logo: Option<&str>) -> String {
    let prefix = escape(prefix);
    let name = escape(name);
    let logo = logo
        .map(|url| format!("<img class=logo src=\"{}\" alt=\"\">", escape(url)))
        .unwrap_or_default();
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="stargate-prefix" content="{prefix}"><title>{name} · Account</title><link rel="stylesheet" href="{prefix}/assets/app.css"><script defer src="{prefix}/assets/app.js"></script></head><body><header>{logo}<a href="{prefix}/">{name}</a><span>Account &amp; access</span></header><main><nav aria-label="Account"><a href="{prefix}/profile">Profile</a><a href="{prefix}/keys">API keys</a><a href="{prefix}/sessions">Sessions</a><button id="logout" hidden>Sign out</button></nav><p id="notice" role="status" aria-live="polite"></p><section id="login" hidden><h1>Your account, connected.</h1><p>Sign in to manage your profile, API keys and sessions.</p><div id="providers"></div></section><section id="profile" hidden><h1>Profile</h1><dl id="identity"></dl></section><section id="keys" hidden><h1>API keys</h1><p>Create credentials for your apps. Each secret is shown once.</p><form id="create-key"><label>Name<input name="name" required maxlength="128" placeholder="Development laptop"></label><label>Scopes<input name="scopes" placeholder="projects:read projects:write"></label><label>Expires at (optional)<input name="expires" type="datetime-local"></label><button>Create key</button></form><div id="new-secret" hidden><p>Copy this key now. It will not be shown again.</p><code id="secret"></code><button id="copy-secret">Copy</button><button id="dismiss-secret">Dismiss</button></div><ul id="key-list"></ul></section><section id="sessions" hidden><h1>Sessions</h1><p>Review where you are signed in and revoke access.</p><button id="revoke-all">Revoke all sessions</button><ul id="session-list"></ul></section></main><footer>Secured by Stargate</footer></body></html>"#
    )
}
