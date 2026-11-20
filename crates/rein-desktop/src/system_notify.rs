//! Channel 3 (status.md §Components 6, §Security; D-062, D-069; E-79): the operating system's
//! own notification, through `notify-rust` — on Linux the freedesktop service over the session
//! bus, on Windows a WinRT toast, on macOS NSUserNotification. The text is one of the app's
//! fixed sentences (notify.rs `sentence` / `merged`): the folder name and a number are the
//! only words not the app's own. Where the system reports a click, the window comes forward.
//!
//! D-069's conditions met here: (1) Windows names the toast with the app's AppUserModelID,
//! so it is not attributed to PowerShell; (2) on macOS the sender is set once, to a fixed
//! bundle id, before the first notification — mac-notification-sys otherwise runs its
//! NSAppleScript lookup (`get_bundle_identifier_or_default`) on the first send; (3) on Linux
//! the body is escaped for the freedesktop body markup, so a folder name is never a tag or a
//! link (Windows sets its text with `SetInnerText`; macOS shows plain text).
//! Each notification waits for its click on a thread of its own: at most one per project per
//! minute (`notify::SystemLimit`). The thread ends when the notification closes; on macOS a
//! notification left unanswered can keep its thread waiting.

use tauri::{AppHandle, Manager};

/// The notification's title line: the app's name.
pub const SUMMARY: &str = "rein-app";
/// The app's identifier (tauri.conf.json), the AppUserModelID of the installed Windows build.
#[cfg_attr(not(windows), allow(dead_code))] // read by the Windows toast only
pub const APP_ID: &str = "io.github.vimurai.rein-app";

/// macOS: the sender the system shows — the value mac-notification-sys's own lookup falls back
/// to, set as a fixed id so the lookup never runs (D-069 condition 2). Our own bundle id can
/// replace it once the window ships as a registered .app.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const MAC_SENDER: &str = "com.apple.Finder";

/// A click on the notification: its body (the default action) or one of its buttons.
pub fn is_click(r: &notify_rust::NotificationResponse) -> bool {
    matches!(
        r,
        notify_rust::NotificationResponse::Default | notify_rust::NotificationResponse::Action(_)
    )
}

/// The body as the freedesktop service is given it: `&`, `<` and `>` escaped, since a service
/// may render body markup (tags and links).
pub fn markup_escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Shows `text` as a system notification. Returns at once; a failure (no notification service,
/// notifications turned off in the system) is traced and otherwise ignored — channels 1 and 2
/// are already done.
pub fn show(app: &AppHandle, text: &str) {
    let app = app.clone();
    let text = text.to_owned();
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        {
            static SENDER: std::sync::Once = std::sync::Once::new();
            SENDER.call_once(|| {
                let _ = notify_rust::set_application(MAC_SENDER);
            });
        }
        let mut n = notify_rust::Notification::new();
        n.appname(SUMMARY).summary(SUMMARY);
        // The freedesktop service reports a click on the body as the "default" action.
        #[cfg(all(unix, not(target_os = "macos")))]
        n.body(&markup_escaped(&text))
            .action("default", "Show rein-app");
        #[cfg(not(all(unix, not(target_os = "macos"))))]
        n.body(&text);
        #[cfg(windows)]
        n.app_id(APP_ID);
        let handle = match n.show() {
            Ok(h) => h,
            Err(e) => {
                crate::status_watch::trace(&format!("system failed: {e}"));
                return;
            }
        };
        // Handed to the system's API (on macOS it is sent in the wait below).
        crate::status_watch::trace("system shown");
        // Every system reports a click on the body as `Default` (Windows' `wait_for_action`
        // would report it as closed).
        let _ = handle.wait_for_response(|r: &notify_rust::NotificationResponse| {
            if is_click(r) {
                crate::status_watch::trace("system clicked");
                forward(&app);
            }
        });
    });
}

/// The window comes forward: shown, restored and focused.
fn forward(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(crate::ipc::MAIN_WINDOW) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use notify_rust::NotificationResponse as R;

    /// A click on the body (Default) or a button brings the window forward; nothing else does.
    #[test]
    fn a_click_is_the_body_or_a_button() {
        assert!(super::is_click(&R::Default));
        assert!(super::is_click(&R::Action("default".into())));
        assert!(!super::is_click(&R::Closed(
            notify_rust::CloseReason::Dismissed
        )));
    }

    /// D-069 condition 3 on Linux: a folder name with markup reaches the service as text.
    #[test]
    fn the_linux_body_is_escaped() {
        assert_eq!(
            super::markup_escaped("<a href=\"x\">p</a> & q: 2 things need you."),
            "&lt;a href=\"x\"&gt;p&lt;/a&gt; &amp; q: 2 things need you."
        );
        assert_eq!(
            super::markup_escaped("&lt;"),
            "&amp;lt;",
            "escaped once, & first"
        );
    }

    /// D-069 condition 1: the Windows toast carries the installed build's AppUserModelID —
    /// the identifier in tauri.conf.json, which the installer gives the app's shortcut.
    #[test]
    fn the_app_id_is_the_apps_identifier() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"].as_str(), Some(super::APP_ID));
    }
}
