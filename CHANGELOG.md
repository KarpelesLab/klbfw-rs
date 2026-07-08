# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `RestContext::with_header`, `with_headers`, `set_header`, and `headers` to
  attach custom headers to every request

## [0.1.2](https://github.com/KarpelesLab/klbfw-rs/compare/v0.1.1...v0.1.2) - 2026-06-30

### Fixed

- restore max_part_size field to keep upload fix non-breaking
- correct AWS S3 multipart upload for files over 64MB

## [0.1.1](https://github.com/KarpelesLab/klbfw-rs/compare/v0.1.0...v0.1.1) - 2026-06-29

### Other

- Stop tracking Cargo.lock (library crate)
- Replace reqwest/ed25519-dalek/url stack with KarpelesLab crates; fix review issues
- Add CI workflow and fix lints to pass it
- Add README badges and bump MSRV to 1.88
- Add release-plz workflow for automated releases
