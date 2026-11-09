//! Fetch, verify and lay out a tagged `rein` release.
//!
//! Blueprint: `.ai/blueprints/distribution.md` §Components 1-3, contract rein D-153 (D-003).
//! Pipeline: [`fetch_release`] → [`verify_release`] → [`install_tree`]. Network access sits
//! behind the [`Http`] trait and signature checking behind [`SignatureVerifier`], so the
//! phase 2 desktop shell reuses the same code path and tests inject fixtures.

mod error;
mod fetch;
mod latest;
mod layout;
mod manifest;
mod matcher;
mod paths;
mod rollback;
mod signature;
mod snapshot;
mod sync;
mod verify;

pub use error::{FetchError, VerifyError};
pub use fetch::{
    HostPolicy, Http, MAX_TARBALL_BYTES, ReleaseSource, ReqwestHttp, asset_names, fetch_release,
};
pub use latest::{LatestRelease, MAX_LATEST_BYTES, REIN_LATEST_API, latest_release, parse_latest};
pub use layout::{
    InstallLock, InstallOutcome, InstallPaths, LayoutError, RunUid, check_owners, install_tree,
    install_tree_locked, install_tree_with_faults, installed_version, owner_guard, uid_of,
};
pub use manifest::{FileEntry, FileMode, Layout, Manifest, ManifestHeader, SUPPORTED_SCHEMA};
pub use matcher::OperatorOwnedMatcher;
pub use paths::check_rel_path;
pub use rollback::{RollbackError, RollbackOutcome, rollback_tree, rollback_tree_with};
pub use signature::{MinisignVerifier, Rejecting, SignatureVerifier, production_verifier};
pub use snapshot::KEEP_SNAPSHOTS;
pub use sync::{Faults, SyncPlan, plan};
pub use verify::{ReleaseAssets, VerifiedRelease, verify_release};
