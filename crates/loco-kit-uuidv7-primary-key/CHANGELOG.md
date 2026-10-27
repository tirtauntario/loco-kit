# Changelog

All notable changes to `loco-kit-uuidv7-primary-key` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Generator templates in `templates/`. Copied into an app's
  `.loco-templates/`, they make `cargo loco generate model`, `migration` and
  `scaffold` write UUID v7 keys and UUID references.

## [0.1.0] - 2026-10-16

First release.

### Added

- `new_id`: a UUID v7 made in Rust, so Postgres, SQLite and MySQL get the
  same time-ordered ids without a database default.
- `UuidV7PrimaryKey::with_uuidv7_pk`, for a model's `before_save`: sets an
  unset single-column primary key to a new id, and leaves set ids and
  composite keys alone.
- `schema`: `loco_rs::schema` with an `add_reference` that adds a UUID
  `<table>_id` column instead of a 64-bit integer.

[Unreleased]: https://github.com/tirtauntario/loco-kit/compare/loco-kit-uuidv7-primary-key-v0.1.0...HEAD
[0.1.0]: https://github.com/tirtauntario/loco-kit/tree/loco-kit-uuidv7-primary-key-v0.1.0
