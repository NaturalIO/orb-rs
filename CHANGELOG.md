# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

### Removed

### Changed

### Fixed

## [0.3.1] - 2025-10-31

### Added

- runtime: Add `AsyncExec::spawn_blocking()`

### Change

- net: Change ResolveAddr trait to async, change bind to async, resolve names in background

## [0.2.0] - 2025-10-30

### Added

- Add net module, which include tcp and unix I/O.
- And UnifyAddr, which smart parser for both socket/path address format.
- Add UnifyStream and UnifyListen to support tcp & unix with the same interface

## [0.1.1] - 2025-10-29

### Added

The first version
