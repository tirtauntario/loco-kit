# Contributing to loco-kit

Bug reports, ideas and pull requests are welcome. Report security issues
privately (see [SECURITY.md](SECURITY.md)), not in public issues. Questions
go to [Discussions](https://github.com/tirtauntario/loco-kit/discussions).

## Before you start

Each kit is small and does one thing. For a bug fix, open a pull request
directly. For a new feature, a new config key or a new kit, open an issue
first so we can agree on scope.

## Development setup

You need Rust 1.94 or newer (`rust-version` in [Cargo.toml](Cargo.toml)). No
database or other service is needed: the tests use SQLite in memory and
temporary folders.

CI runs the first three of these; run all four before a pull request:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps --workspace    # should build without warnings
```

Run one crate's tests with `cargo test -p loco-kit-credentials`.

CI also checks the build on Rust 1.94, and once a week runs the tests against
the newest compatible dependencies (`cargo update && cargo test`), so a new
loco-rs 1.x release that breaks a kit shows up early.

To try a change in a real app, see
[Developing alongside an app](README.md#developing-alongside-an-app).

## Pull requests

- Keep each pull request to one change, with tests. A bug fix needs a test
  that fails without it.
- Update the crate's README when behavior or setup changes, and add a line
  under `## [Unreleased]` in the crate's `CHANGELOG.md`.
- Make sure fmt, clippy and the tests pass.

## Adding a kit

A kit is a crate under `crates/`, named `loco-kit-<name>`. The rules:

- It depends on Loco (and ordinary crates) only, never on a particular app.
- It fills one gap in Loco. If Loco already does it, it is not a kit.
- It takes `loco-rs` from `[workspace.dependencies]`, so all kits move to a
  new Loco version together.
- It inherits `edition`, `license`, `authors`, `repository` and
  `rust-version` from the workspace, and has its own `description`,
  `keywords`, `categories`, `README.md` and `CHANGELOG.md`.
- Its README stands alone: what it does and why, how to install it, setup
  steps in order, a config example, usage, edge cases and limits.
- It reads settings from the Loco config where it can. Where it must read the
  process environment (because no config exists yet), the README says so.
- It gets a row in the crate table of the root README.

## Releases

The maintainer releases. Each crate is versioned and tagged on its own
(`<crate>-v<version>`) with
[cargo-release](https://github.com/crate-ci/cargo-release), configured in
[release.toml](release.toml). Release one crate at a time:

```sh
cargo release minor -p loco-kit-credentials            # dry run: shows what it would do
cargo release minor -p loco-kit-credentials --execute
```

This moves the `## [Unreleased]` entries in the crate's `CHANGELOG.md` under
the new version and date, bumps the version, commits, tags
`loco-kit-credentials-v0.2.0` and pushes the commit and tag. The crates are
not published to crates.io yet (`publish = false`).

The kits follow semantic versioning. While a crate is at 0.x, a breaking
change bumps the minor version.

## License

By contributing, you agree that your contributions are dual licensed under
MIT OR Apache-2.0, as described in the [README](README.md#license), without
any additional terms or conditions.
