// The IPC command table (desktop.md §API). One list, `include!`d by build.rs — which turns
// it into the `allow-<command>` permissions the capability file must grant — and by ipc.rs.
// Adding a command: add it here, grant it in capabilities/main.json, implement it in ipc.rs.
pub const COMMANDS: &[&str] = &["tab_open", "pty_write", "pty_resize", "pty_ack", "tab_detach"];
