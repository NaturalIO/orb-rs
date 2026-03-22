# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

### Removed

### Changed

### Fixed

## [0.12.1] - 2026-03-22

### Added

- worker_pool: Add scalable bounded/unbound worker pools which support async & blocking (under `worker` feature)

## [0.11.1] - 2026-03-21

### Fixed

- orb-smol: Fix global flag broken in 0.11.0

### Removed

- orb-smol: Remove SmolExec::new_global(), should use SmolRT::multi() instead.

## [0.11.0] - 2026-03-21

### Added

- AsyncRuntime: Refactor to Add static spawn method.

- Add SmolExec & TokioExec to impl AsyncExec.

## Changed

- AsyncRuntime no longer inherit AsyncExec, instead it as associate type "Exec" of AsyncRuntime.

- Move current()/one()/multi() will return AsyncExec.

- Fix missing self in AsyncExec::spawn_blocking()

- Rename trait AsyncHandle -> AsyncJoiner, ThreadHandle -> ThreadJoiner

### Removed

- orb-smol: Remove new_with_executor() because it does not ensure thread-local set

- AsyncExecDyn is removed, because we can not use static method to spawn.

## [0.10.0] - 2026-03-18

### Added

- AsyncExec: Add Clone()

### Removed

- Remove blanket Deref inherit of AsyncRuntime/AsyncTime/AsyncIO/AsyncExec

## [0.9.0] - 2026-03-18

### Added

- AsyncExec: Add current(), one(), multi() with unified tested behavior

## [0.8.0] - 2026-03-16

### Added

- AsyncExecDyn: for spawn_detach with trait object

## [0.7.0] - 2026-03-14

### Added

- AsyncHandle: Add detach_boxed() and abort_boxed() to support dyn trait object

## [0.6.0] - 2026-02-01

### Fixed

- AsyncTime: Rename AsyncTime::tick() to AsyncTime::interval() to prevent confuse with tick(&self)

- AsyncTime: tick() should not consume self

- net: Change connect_unix() arg from &PathBuf to &Path

## [0.5.0] - 2025-11-01

### Added

- Add AsyncHandle and ThreadHandle to AsyncExec associate types

## [0.4.0] - 2025-10-31

### Added

- orb-smol: Add `unwind` feature, to catch panic inside the task

- orb-tokio:
  - TokioRT created with Handle not allowed to call `block_on`.
  - Add Clone to TokioRT

- AsyncJoinHandle: Add is_finished()

### Changed

- runtime:
  - Unify the drop behavior of AsyncJoinHandle, to aligned with tokio.
  - `spawn_blocking()` should return ThreadJoinHandle
  - Remove `join()` method, handle itself should be a future

## [0.3.2] - 2025-10-31

### Fixed

- net: Fix `bind(&addr)` not recognized when addr: &str

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
