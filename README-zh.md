# Orb

[![Crates.io](https://img.shields.io/crates/v/orb.svg)](https://crates.io/crates/orb)
[![Documentation](https://docs.rs/orb/badge.svg)](https://docs.rs/orb)

Orb 是一个用于编写运行时无关的异步 Rust 代码的抽象层，允许您编写可在不同异步运行时（如 `tokio` 或 `smol`）之间工作的代码。

[English](README.md) | [中文](README-zh.md)

## 特性

- 运行时无关：编写可与多个异步运行时配合使用的代码
    - 行为更符合 tokio 的使用习惯，避免因为实际 runtime 行为差异造成的bug (比如， drop 一个 AsyncJoinHandle 默认是 detach 而不是 cancel.)
- 可扩展：易于实现对新运行时的支持, 无须改动 orb 仓库本身
- 网络功能：
    - 提供了额外的统一类型抽象，如 (tcp + unix) `UnifyListener` / `UnifyStream`
    - 通过 `ResolveAddr` trait 提供非阻塞域名解析


## 开发目标

主要目标是将应用程序逻辑与特定的异步运行时实现解耦，使您能够：

- 编写可移植的异步代码，可在多个运行时组合中同时工作
- 在不更改核心逻辑的情况下切换到新的运行时
- 使用不同的运行时特性测试代码
- 提升异步网络编程的体验。

这是在开发 [razor-rpc](https://docs.rs/razor-rpc) 过程中的一个副产品。缘由是：

- 目前对于不同的运行时设计没有现行标准，在开发共享库的时候往往只针对特定的运行时进行适配。
- 在代码中使用太多 `#[cfg(feature=xxx)]` 会让代码难以阅读。
- 像 `smol` 这样的运行时生态系统允许您自定义执行器，但由于使用方式不同，学习成本对于习惯tokio的人比较高，并且缺乏常用工具函数（例如，`async-io` 或 `smol` 中没有 `timeout` 函数）。
- 在多层 cargo 依赖关系中传递功能特性更加困难。这就是我们在这个项目中不用 feature 来区分runtime 的原因。
- 如果用户想为自己的需求定制运行时，他们会面临生态系统支持不完整的困境。
- 某些项目（如 Hyper）定义了抽象层，但每个项目都单独这样做是重复劳动。

## 使用方法

要使用 Orb，您需要同时依赖核心 `orb` crate 和一个运行时适配器 crate，如 `orb-tokio` 或 `orb-smol`。

在您的 `Cargo.toml` 中：

```toml
[dependencies]
orb = "0"
orb-tokio = "0"
orb-smol = "0"
```

在 crate 级别有一个全局 trait `AsyncRuntime` 组合了所有功能，添加 `use orb::prelude::*` 将导入您需要的所有 trait。

`new()` 函数有一些变体，也请参考子 crate 中的文档：

- [orb-tokio](https://crates.io/crates/orb-tokio) - 适用于 Tokio 运行时
- [orb-smol](https://crates.io/crates/orb-smol) - 适用于 Smol 运行时

## 许可证

本项目采用 MIT 许可证 - 详情请见 LICENSE 文件。
