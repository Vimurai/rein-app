//! Enumerate Claude profiles and merge the closed set of Rein-owned settings keys.
//!
//! Blueprint: `.ai/blueprints/distribution.md` §Components 6 and §Security trust
//! boundary 3; contract rein §2.3 / D-132; marketplace payload D-002; plugin enablement
//! through `claude plugin` D-005.
//! Seams for phase 2: [`enumerate`], [`plan`], [`apply`] (+ [`undo`]), [`set_plugins`].

mod patch;
mod plugins;
mod profile;
pub mod rein_plugins;
mod store;

pub use patch::{AllowedKey, KeyChange, Patch, PlanError, plan};
pub use plugins::{
    PluginAction, PluginError, ProcessShell, Shell, ShellOutput, check_plugins,
    marketplace_add_argv, plugin_argv, register_marketplace, set_plugins,
};
pub use profile::{Enumeration, Profile, enumerate};
pub use store::{
    ApplyError, ApplyOptions, Outcome, apply, ensure_backup, read_settings, resolve_settings_path,
    undo,
};
