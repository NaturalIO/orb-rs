# Orb

[![Crates.io](https://img.shields.io/crates/v/orb.svg)](https://crates.io/crates/orb)
[![Documentation](https://docs.rs/orb/badge.svg)](https://docs.rs/orb)

Orb is an abstraction layer for writing runtime-agnostic async Rust code, allowing you to write code that works with different async runtimes, like `tokio` or `smol`.

We took the name `Orb` because it gets around :)

[English](README.md) | [中文](README-zh.md)

## Features

- **Runtime Agnostic**: Write code that works with multiple async runtimes
- **Trait-Based**: Clean, well-defined interfaces for async operations
- **Extensible**: Easy to implement support for new runtimes
- **Lightweight**: No overhead abstraction layer

## The goal

The main goal is to decouple your application logic from specific async runtime implementations, allowing you to:

- Write portable async code that works at the same time in combination of multiple runtimes
- Switch to new runtimes without changing your core logic
- Test your code with different runtime characteristics


This is a side project during the development of [razor-rpc](https://docs.rs/razor-rpc). Because:

- There is no established standard for designing different runtimes, when developing shared libraries, developers often only target specific runtimes.
- Using too many `#[cfg(feature=xxx)]` in code makes it hard to read.
- Runtimes like `smol` ecology enable you to customize executors, but there's high learning cost, and lack utility functions (for example, there's no `timeout` function in `async-io` or `smol`).
- Passing features through sub-projects through multiple layers of cargo dependencies is even more difficult. (that's why we don't use feature in this crate)
- If users want to customize a runtime for their own needs, they face the dilemma of incomplete ecosystem support.
- Some projects like Hyper define abstraction layers, having each project do this individually is a huge maintenance cost.

This is why this crate was written.

## Usage

To use Orb, you need to depend on both the core `orb` crate and a runtime adapter crate like `orb-tokio` or `orb-smol`.

In your `Cargo.toml`:

```toml
[dependencies]
orb = "0"
orb-tokio = "0"
orb-smol = "0"
```

There's a global trait `AsyncRuntime` that combines all features at the crate level, and adding `use orb::prelude::*` will import all the traits you need.

There are some variants of the `new()` function, also refer to the documentation in the sub-crates:

- [orb-tokio](https://crates.io/crates/orb-tokio) - For the Tokio runtime
- [orb-smol](https://crates.io/crates/orb-smol) - For the Smol runtime

## License

This project is licensed under the MIT License - see the LICENSE file for details.
