# General
- All comment must be in English. Omit unnecessary comment.
- If you don't know 3rd-party API, should lookup on `https://docs.rs/<crate>`.
- All trait with async function, should define with  return type of `impl Future + Send`, according to rust 1.75
- Use short form when reference to namespace as much as possible.

# test
- Tests don't depend of a runtime, can be written in @tests dir
- Tests for specified async runtime trait,  should be written in @test-utils, which can be call from all runtime implement in the sub crates (refer to @tokio, @smol).
- The test should use rstest instead of test, with fixture `setup` to setup the logger, in confirm with other test.
