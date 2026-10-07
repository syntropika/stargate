//! Production assets are compiled into the Rust artifact.
pub const JAVASCRIPT: &str = include_str!("../assets/app.js");
pub const STYLESHEET: &str = include_str!("../assets/app.css");
pub const LOGO: &str = include_str!("../assets/stargate.svg");

pub struct Page<'a> {
    pub prefix: &'a str,
    pub name: &'a str,
    pub logo: Option<&'a str>,
    pub stylesheet: Option<&'a str>,
    pub providers: Vec<&'a str>,
    pub route: &'a str,
    pub local: bool,
}

pub fn account_page(page: Page<'_>) -> String {
    let config = serde_json::json!({
        "prefix": page.prefix, "name": page.name, "logo": page.logo,
        "providers": page.providers, "page": page.route.trim_matches('/'),
        "local": page.local,
    })
    .to_string()
    .replace('<', "\\u003c")
    .replace('&', "\\u0026");
    let prefix = escape(page.prefix);
    let name = escape(page.name);
    let favicon = page
        .logo
        .map(escape)
        .unwrap_or_else(|| format!("{prefix}/assets/stargate.svg"));
    let stylesheet = page
        .stylesheet
        .map(|url| format!("<link rel=\"stylesheet\" href=\"{}\">", escape(url)))
        .unwrap_or_default();
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="stargate-prefix" content="{prefix}"><title>{name} · Account</title><link rel="icon" href="{favicon}"><link rel="stylesheet" href="{prefix}/assets/app.css">{stylesheet}<script defer src="{prefix}/assets/app.js"></script></head><body><div id="stargate-root"><main aria-busy="true"><h1>{name}</h1><p role="status">Loading your account…</p><noscript>Enable JavaScript to manage your account.</noscript></main></div><script id="stargate-config" type="application/json">{config}</script></body></html>"#
    )
}

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
    account_page(Page {
        prefix,
        name,
        logo,
        stylesheet,
        providers: vec![],
        route: "",
        local: false,
    })
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

    #[test]
    fn public_configuration_cannot_escape_the_json_script() {
        let name = "Example </script><script>alert('x')</script> & team";
        let html = account_page(Page {
            prefix: "/account",
            name,
            logo: Some("https://example.com/logo.png?x=1&y=2"),
            stylesheet: None,
            providers: vec!["team \"oidc\""],
            route: "/keys/",
            local: false,
        });
        let marker = "<script id=\"stargate-config\" type=\"application/json\">";
        let config = html
            .split_once(marker)
            .unwrap()
            .1
            .split_once("</script>")
            .unwrap()
            .0;
        assert!(!config.contains('<'));
        assert!(!config.contains('&'));
        let value: serde_json::Value = serde_json::from_str(config).unwrap();
        assert_eq!(value["name"], name);
        assert_eq!(value["prefix"], "/account");
        assert_eq!(value["page"], "keys");
        assert_eq!(value["providers"][0], "team \"oidc\"");
        assert!(html.contains("src=\"/account/assets/app.js\""));
        assert!(html.contains("href=\"/account/assets/app.css\""));
        assert!(!html.contains("<script>alert"));
    }
}
