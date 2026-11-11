# rein-app

**rein-app is the installer and updater for [Rein](https://github.com/Vimurai/rein).** It
fetches a signed `rein` release, verifies it, installs it into `$HOME/.rein`, and runs
`rein install` and an install check (`rein doctor --scope install`) from it. It can also update or roll back that install and set
the few Claude Code profile settings Rein owns.

rein-app **is not** Rein. It contains no Rein logic: the `rein` CLI, its MCP servers, hooks
and gates all come from the `rein` release it installs. It never edits a project, and it
writes no file inside one.

> **Status: no release has been published yet.** rein-app can only install a signed
> `rein` release, and none exists yet. Nothing below is available as a download today.

## Install channels (planned)

These are planned; none is published yet:

- **macOS and Linux:** a Homebrew tap.
- **Debian and Ubuntu:** a `.deb` package.
- **Windows:** a launcher that runs Rein inside **WSL2**. Native Windows is not supported.

## Build from source

You need a stable Rust toolchain.

```sh
git clone https://github.com/Vimurai/rein-app
cd rein-app
cargo build --release
./target/release/rein-app --help
```

Every build, a source build included, has the Rein release public key compiled in from
[`docs/contract/release-key.pub`](docs/contract/release-key.pub), a copy of the key Rein
will sign its releases with. rein-app never fetches a key: it installs only a release whose
signature matches that key, and fails closed on anything else. Building with the
`reject-all-signatures` feature (`cargo build --release --features
rein-release/reject-all-signatures`) gives a binary that trusts no key and refuses every
release.

## Documentation

- [Operator guide](docs/operator-guide.md): every command and exit code, what an install
  touches, snapshots and rollback, profile settings, and the refusals you can meet.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
