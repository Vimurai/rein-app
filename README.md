# rein-app

**rein-app is the installer and updater for [Rein](https://github.com/Vimurai/rein).** It
fetches a signed `rein` release, verifies it, installs it into `$HOME/.rein`, and runs
`rein install` and `rein doctor` from it. It can also update or roll back that install and set
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

A release build is built with Rein's release public key baked in (no such key exists
yet). **A source build without that key refuses every release**: it fails closed and installs nothing. It is useful for
development and for running the tests (`cargo test --workspace`), but it cannot install Rein.

## Documentation

- [Operator guide](docs/operator-guide.md): every command and exit code, what an install
  touches, snapshots and rollback, profile settings, and the refusals you can meet.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
