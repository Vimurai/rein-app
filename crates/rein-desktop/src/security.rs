//! Web view → core boundary (desktop.md §Security 1): the web view loads only the bundled
//! UI; every navigation elsewhere and every new window is refused.

use tauri::Url;

/// The origin the bundled UI is served from: `tauri://localhost` on Linux and macOS,
/// `http(s)://tauri.localhost` where the webview cannot register a custom scheme (Windows).
pub fn navigation_allowed(url: &Url) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => url.host_str() == Some("tauri.localhost"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_bundled_origin_is_navigable() {
        for ok in ["tauri://localhost/index.html", "http://tauri.localhost/"] {
            assert!(navigation_allowed(&Url::parse(ok).unwrap()), "{ok}");
        }
        for bad in [
            "https://example.com/",
            "http://localhost:1420/",
            "file:///etc/passwd",
            "tauri://evil/",
            "http://tauri.localhost.evil.com/",
            "javascript:alert(1)",
            "data:text/html,x",
        ] {
            assert!(!navigation_allowed(&Url::parse(bad).unwrap()), "{bad}");
        }
    }
}
