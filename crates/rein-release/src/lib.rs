//! Fetch, verify and lay out a tagged `rein` release.
//!
//! Blueprint: `.ai/blueprints/distribution.md` §Components 1-3. The fetcher, the
//! fail-closed verifier (E-3) and the layout writer (E-5) land here behind `Http`/`Fs`
//! traits so the phase 2 desktop shell reuses the same code path.
