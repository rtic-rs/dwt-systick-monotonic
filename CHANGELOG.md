# Change Log

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

For each category, *Added*, *Changed*, *Fixed* add new entries at the top!

## [Unreleased]

### Added

- CI: Add clippy

### Fixed

### Changed

- CI: Use native GHA rustup and cargo

## [v1.1.0] - 2022-10-05

### Added

- CI changelog entry enforcer
- New feature `extend` to track DWT cycle counter overflows and extend
  the range to `u64`.

## [v1.0.0] - 2021-12-25

- Edition 2021

## [v0.1.0] - 2021-02-18

Initial release

[Unreleased]: https://github.com/rtic-rs/dwt-systick-monotonic/compare/v1.1.0...HEAD
[v1.1.0]: https://github.com/rtic-rs/dwt-systick-monotonic/compare/v1.0.0...v1.1.0
[v1.0.0]: https://github.com/rtic-rs/dwt-systick-monotonic/compare/v0.1.0...v1.0.0
[v0.1.0]: https://github.com/rtic-rs/dwt-systick-monotonic/compare/f491196...v0.1.0
