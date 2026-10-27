# Changelog

All notable changes to `loco-kit-credentials` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-16

First release.

### Added

- Encrypted credentials files, decrypted at boot: `config/credentials.yml.enc`
  with `config/master.key`, or one file per environment,
  `config/credentials/<env>.yml.enc` with `config/credentials/<env>.key`. The
  per-environment file replaces the global one. `LOCO_MASTER_KEY` overrides
  the key file, and `LOCO_CONFIG_FOLDER` moves the folder.
- `load_config`, for `Hooks::load_config`: loads the Loco config and replaces
  every `credential:<path>` string with that credential. A missing credential
  stops boot.
- `install`, for `Hooks::after_context`, and `Credentials::from_context`,
  `get` and `get_opt` to read any credential by dotted path from code.
- `cargo loco credentials edit` and `show`, with `-e <env>`, through
  `cli::run` in `src/bin/main.rs`. They run without booting the app. The
  first edit of a file generates its key.
- AES-256-GCM encryption with a fresh nonce on every write, public in the
  `cipher` module.

[Unreleased]: https://github.com/tirtauntario/loco-kit/compare/loco-kit-credentials-v0.1.0...HEAD
[0.1.0]: https://github.com/tirtauntario/loco-kit/tree/loco-kit-credentials-v0.1.0
