//! Platform helpers shared by the `rein-app` CLI and the `rein-desktop` window, so each
//! behaviour has one implementation (desktop.md §Windows window W2, D-038): the bounded
//! process run (output cap and time limit, D-032) and WSL state detection.

pub mod process;
pub mod wsl;
