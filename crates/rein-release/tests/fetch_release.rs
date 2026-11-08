//! Fetcher behaviour against the local fixture server: the real `ReqwestHttp` code path,
//! no network beyond 127.0.0.1 (blueprint §Execution Constraints).

mod support;

use std::fs;

use rein_release::{
    FetchError, HostPolicy, ReleaseSource, ReqwestHttp, fetch_release, verify_release,
};
use semver::Version;
use support::*;

fn version() -> Version {
    Version::parse(VERSION).unwrap()
}

#[test]
fn fetched_release_verifies_end_to_end() {
    let key = TestKey::generate();
    let base = serve(Release::good(&key).routes());
    let tmp = tempfile::tempdir().unwrap();
    let http = ReqwestHttp::new(HostPolicy::loopback_only()).unwrap();
    let assets = fetch_release(
        &http,
        &ReleaseSource { base_url: base },
        &version(),
        tmp.path(),
    )
    .unwrap();
    let verified = verify_release(
        &assets,
        &key.verifier(),
        &version(),
        &tmp.path().join("tree"),
    )
    .unwrap();
    assert_eq!(verified.manifest.files.len(), good_files().len());
}

#[test]
fn missing_asset_fails_and_removes_partial_downloads() {
    let key = TestKey::generate();
    let mut routes = Release::good(&key).routes();
    routes.retain(|path, _| !path.ends_with("manifest.json"));
    let base = serve(routes);
    let tmp = tempfile::tempdir().unwrap();
    let http = ReqwestHttp::new(HostPolicy::loopback_only()).unwrap();
    let err = fetch_release(
        &http,
        &ReleaseSource { base_url: base },
        &version(),
        tmp.path(),
    )
    .unwrap_err();
    assert!(
        matches!(err, FetchError::Status { status: 404, .. }),
        "{err}"
    );
    assert!(
        fs::read_dir(tmp.path()).unwrap().next().is_none(),
        "partial files left behind"
    );
}

#[test]
fn redirect_to_a_disallowed_host_is_refused() {
    let key = TestKey::generate();
    let mut routes = Release::good(&key).routes();
    let tar_path = format!("/v{VERSION}/rein-{VERSION}.tar.gz");
    routes.insert(
        tar_path,
        Route::Redirect("http://10.255.255.1/evil.tar.gz".into()),
    );
    let base = serve(routes);
    let tmp = tempfile::tempdir().unwrap();
    let http = ReqwestHttp::new(HostPolicy::loopback_only()).unwrap();
    let err = fetch_release(
        &http,
        &ReleaseSource { base_url: base },
        &version(),
        tmp.path(),
    )
    .unwrap_err();
    assert!(
        matches!(err, FetchError::Network(ref m) if m.contains("disallowed")),
        "{err}"
    );
}

#[test]
fn production_policy_refuses_plain_http_before_connecting() {
    let base = serve(Default::default());
    let tmp = tempfile::tempdir().unwrap();
    let http = ReqwestHttp::new(HostPolicy::github()).unwrap();
    let err = fetch_release(
        &http,
        &ReleaseSource { base_url: base },
        &version(),
        tmp.path(),
    )
    .unwrap_err();
    assert!(matches!(err, FetchError::HostNotAllowed(_)), "{err}");
}
