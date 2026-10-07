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
    page_with_stylesheet(prefix, name, logo, None)
}

pub fn page_with_stylesheet(
    prefix: &str,
    name: &str,
    logo: Option<&str>,
    stylesheet: Option<&str>,
) -> String {
    let prefix = escape(prefix);
    let name = escape(name);
    let logo = logo
        .map(|url| format!("<img class=logo src=\"{}\" alt=\"\">", escape(url)))
        .unwrap_or_default();
    let stylesheet = stylesheet
        .map(|url| format!("<link rel=\"stylesheet\" href=\"{}\">", escape(url)))
        .unwrap_or_default();
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="stargate-prefix" content="{prefix}"><title>{name} · Account</title><link rel="stylesheet" href="{prefix}/assets/app.css">{stylesheet}<script defer src="{prefix}/assets/app.js"></script></head><body><header>{logo}<a href="{prefix}/">{name}</a><span>Account &amp; access</span></header><main><nav aria-label="Account"><a href="{prefix}/profile">Profile</a><a href="{prefix}/keys">API keys</a><a href="{prefix}/sessions">Sessions</a><button id="logout" hidden>Sign out</button></nav><p id="notice" role="status" aria-live="polite"></p><section id="login" hidden><h1>Your account, connected.</h1><p>Sign in to manage your profile, API keys and sessions.</p><div id="providers"></div></section><section id="profile" hidden><h1>Profile</h1><dl id="identity"></dl></section><section id="keys" hidden><h1>API keys</h1><p>Create credentials for your apps. Each secret is shown once.</p><form id="create-key"><label>Name<input name="name" required maxlength="128" placeholder="Development laptop"></label><label>Scopes<input name="scopes" placeholder="projects:read projects:write"></label><label>Expires at (optional)<input name="expires" type="datetime-local"></label><button>Create key</button></form><div id="new-secret" hidden><p>Copy this key now. It will not be shown again.</p><code id="secret"></code><button id="copy-secret">Copy</button><button id="dismiss-secret">Dismiss</button></div><ul id="key-list"></ul></section><section id="sessions" hidden><h1>Sessions</h1><p>Review where you are signed in and revoke access.</p><button id="revoke-all">Revoke all sessions</button><ul id="session-list"></ul></section></main><footer>Secured by Stargate</footer></body></html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_theme_is_escaped_and_loaded_after_the_base_stylesheet() {
        let html = page_with_stylesheet(
            "/auth",
            "Example",
            None,
            Some("https://app.example.com/theme.css?x=\"&y=<bad>"),
        );
        assert!(
            html.contains("href=\"https://app.example.com/theme.css?x=&quot;&amp;y=&lt;bad&gt;\"")
        );
        assert!(html.find("/assets/app.css").unwrap() < html.find("/theme.css").unwrap());
        assert!(!html.contains("<bad>"));
    }

    #[test]
    fn the_original_page_helper_retains_the_default_ui() {
        assert_eq!(
            page("/auth", "Example", None),
            page_with_stylesheet("/auth", "Example", None, None)
        );
    }
}
