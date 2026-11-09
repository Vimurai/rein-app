//! Fetch, verify and lay out a tagged `rein` release.
//!
//! Blueprint: `.ai/blueprints/distribution.md` §Components 1-3, contract rein D-153 (D-003).
//! Pipeline: [`fetch_release`] → [`verify_release`] → [`install_tree`]. Network access sits
//! behind the [`Http`] trait and signature checking behind [`SignatureVerifier`], so the
//! phase 2 desktop shell reuses the same code path and tests inject fixtures.

mod error;
mod fetch;
mod layout;
mod manifest;
mod matcher;
mod paths;
mod signature;
mod verify;

pub use error::{FetchError, VerifyError};
pub use fetch::{
    HostPolicy, Http, MAX_TARBALL_BYTES, ReleaseSource, ReqwestHttp, asset_names, fetch_release,
};
pub use layout::{
    InstallLock, InstallOutcome, InstallPaths, KEEP_PREVIOUS, LayoutError, install_tree,
    install_tree_locked, installed_version,
};
pub use manifest::{FileEntry, FileMode, Layout, Manifest, ManifestHeader, SUPPORTED_SCHEMA};
pub use matcher::OperatorOwnedMatcher;
pub use paths::check_rel_path;
pub use signature::{MinisignVerifier, Rejecting, SignatureVerifier, production_verifier};
pub use verify::{ReleaseAssets, VerifiedRelease, verify_release};
