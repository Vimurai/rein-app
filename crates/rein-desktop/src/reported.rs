//! What only the web side can tell the core (status.md §API, D-062; E-76): the tab in front of
//! the user, the bells its terminals rang, and its last `attention_items` input (the tabs'
//! closed booleans and the install health). The window's focus comes from the core's own
//! window event. The core keeps the bell count by itself so the status and "Needs you" hold
//! for every tab, and the status poll can use them while the window is hidden.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::State;

use crate::attention::{AttentionState, Health, TabIn};
use crate::ipc::Desktop;
use crate::status_watch::StatusWatch;

pub struct Reported(Mutex<Inner>);

struct Inner {
    current: Option<u32>,
    focused: bool,
    bells: HashMap<u32, u32>,
    tabs_in: Vec<TabIn>,
    health: Health,
}

impl Default for Reported {
    fn default() -> Self {
        Self(Mutex::new(Inner {
            current: None,
            focused: true,
            bells: HashMap::new(),
            tabs_in: Vec::new(),
            health: Health::Unknown,
        }))
    }
}

impl Reported {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A bell in `tab`'s terminal: marked unless the tab is in front of the user (the current
    /// tab of a focused window), as the web side marked it before E-76.
    pub fn bell(&self, tab: u32) {
        let mut r = self.lock();
        if !(r.focused && r.current == Some(tab)) {
            *r.bells.entry(tab).or_default() += 1;
        }
    }

    /// The current tab changed; its bells are seen once the window has focus.
    pub fn current(&self, tab: Option<u32>) {
        let mut r = self.lock();
        r.current = tab;
        seen(&mut r);
    }

    /// The window gained or lost focus.
    pub fn focused(&self, focused: bool) {
        let mut r = self.lock();
        r.focused = focused;
        seen(&mut r);
    }

    /// Bells rung in `tab` while it was out of sight and not yet looked at.
    pub fn bells(&self, tab: u32) -> u32 {
        self.lock().bells.get(&tab).copied().unwrap_or(0)
    }

    /// The web side's last `attention_items` input.
    pub fn input(&self, tabs: Vec<TabIn>, health: Health) {
        let mut r = self.lock();
        r.tabs_in = tabs;
        r.health = health;
    }

    pub fn last_input(&self) -> (Vec<TabIn>, Health) {
        let r = self.lock();
        (r.tabs_in.clone(), r.health)
    }

    /// Tabs no longer open keep no bells.
    pub fn retain(&self, open: &[u32]) {
        self.lock().bells.retain(|t, _| open.contains(t));
    }

    /// The web side said the tab's tmux setup was refused (its last `attention_items`).
    pub fn refused(&self, tab: u32) -> bool {
        self.lock()
            .tabs_in
            .iter()
            .any(|t| t.tab == Some(tab) && t.refused)
    }
}

fn seen(r: &mut Inner) {
    if let Some(t) = r.current.filter(|_| r.focused) {
        r.bells.remove(&t);
    }
}

/// `tab_current(tab)`: the tab in front of the user changed (none: no terminal tab).
#[tauri::command]
pub fn tab_current(
    attention: State<'_, AttentionState>,
    watch: State<'_, StatusWatch>,
    tab: Option<u32>,
) {
    attention.reported.current(tab);
    watch.kick();
}

/// `tab_bell(tab)`: the tab's terminal rang its bell (the core does not parse the output: a
/// BEL byte also ends an OSC sequence, E-76 read (c)). Unknown tabs are ignored.
#[tauri::command]
pub fn tab_bell(
    state: State<'_, Desktop>,
    attention: State<'_, AttentionState>,
    watch: State<'_, StatusWatch>,
    tab: u32,
) {
    if state.tab_target(tab).is_some() {
        attention.reported.bell(tab);
        watch.kick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Moved from the web side (attention.ts bellMarks / bellClears, E-61): a bell marks its
    /// tab unless that tab is in front of the user; the mark goes once it is.
    #[test]
    fn a_bell_marks_a_tab_out_of_sight_until_it_is_in_front() {
        let a = Reported::default();
        a.current(Some(1));
        a.bell(1);
        assert_eq!(a.bells(1), 0, "the current tab of a focused window");
        a.bell(2);
        a.bell(2);
        assert_eq!(a.bells(2), 2, "a background tab counts its bells");
        a.focused(false);
        a.bell(1);
        assert_eq!(a.bells(1), 1, "the window has no focus");
        a.focused(true);
        assert_eq!(a.bells(1), 0, "seen once the window has focus again");
        a.current(Some(2));
        assert_eq!(a.bells(2), 0, "seen once it is the current tab");
    }

    #[test]
    fn the_last_input_and_refused() {
        let a = Reported::default();
        assert!(!a.refused(3));
        let t = |tab, refused| TabIn {
            index: 0,
            tab: Some(tab),
            refused,
            update: false,
        };
        a.input(vec![t(3, true), t(4, false)], Health::Warn);
        assert!(a.refused(3) && !a.refused(4));
        assert_eq!(a.last_input().1, Health::Warn);
    }
}
