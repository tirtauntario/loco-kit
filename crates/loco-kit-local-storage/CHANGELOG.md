# Changelog

All notable changes to `loco-kit-local-storage` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-16

First release.

### Added

- `install`, for `Hooks::after_context`: builds `ctx.storage` from the
  `initializers.storage` config block, with a `local` driver (a folder,
  created when missing) or a `memory` driver. A missing or invalid block
  stops boot.
- `StorageKey`: random keys of 32 lowercase hex characters, stored at
  `ab/cd/<key>` so no directory holds more than a small share of the files.
  `StorageKey::parse` rejects anything else, so a stored key cannot name
  another path.
- `StorageCheck`, an initializer that makes `cargo loco doctor` write, read
  back and delete a probe file.

[Unreleased]: https://github.com/tirtauntario/loco-kit/compare/loco-kit-local-storage-v0.1.0...HEAD
[0.1.0]: https://github.com/tirtauntario/loco-kit/tree/loco-kit-local-storage-v0.1.0
