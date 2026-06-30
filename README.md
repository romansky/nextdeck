# cargo-test-tui

Terminal-native Rust test UI built on cargo-nextest.

## Nextest integration

This prototype deliberately avoids parsing human terminal output.

- Discovery uses `nextest-metadata::ListCommand`, which runs `cargo nextest list --message-format=json` and deserializes to nextest's own `TestListSummary`.
- Live runs use nextest's experimental structured runner output:
  `NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 cargo nextest run --message-format libtest-json-plus --message-format-version 0.1`.
- Human progress output is disabled for run commands so stdout is reserved for JSON lines.

As of the current nextest docs, first-class newline-delimited JSON run events are still future work. The `libtest-json-plus` adapter is isolated in `src/nextest.rs` so it can be replaced if nextest adds a native event stream.

Current cargo-nextest emits per-test names in the form `crate::binary$module::test`, while `nextest-metadata` discovery reports `module::test`. The adapter normalizes the prefix away. If two test binaries expose the same test path, events are currently applied by name to both; a future version should keep a suite-event context or ask nextest for a per-test binary identifier.

## Usage

```sh
cargo run -- --run
```

Non-interactive discovery smoke:

```sh
cargo run -- --list-json
```

Theme mode defaults to terminal background detection:

```sh
cargo run -- --theme auto
cargo run -- --theme dark
cargo run -- --theme light
```

## Local Publishing

Project automation lives in `xtask` and is available through the Cargo alias:

```sh
cargo xtask --help
```

Useful local publishing commands:

- `cargo xtask check --allow-dirty`: run format check, tests, and package verification.
- `cargo xtask package --allow-dirty`: create and verify `target/package-verify/package/cargo-test-tui-*.crate`.
- `cargo xtask publish-local --allow-dirty`: package, install from the verified package directory, and verify `PATH` resolves to the installed binary.
- `cargo xtask install-path`: install directly from the workspace checkout.

Keys:

- `Up` / `Down`: move selection
- `PageUp` / `PageDown`: page the focused pane
- `Home` / `End`: jump to the start/end of the focused pane
- `Left` / `Right`: collapse or expand
- `Enter` / `Space`: toggle expansion
- `Tab`: switch focus between tree and output
- `r`: run selected scope, or all tests from the workspace node
- `R`: rerun failed tests
- `f` / `F`: jump to next/previous failure
- `h` / `?` / `F1`: show help
- `/`: search
- `q`: quit

## Current checkpoint

Implemented:

- Scoped run model for workspace, package, module, single test, and failed-test reruns.
- Package scopes use nextest/Cargo's native `-p <package>` argument.
- Module, test, and failed scopes use nextest string filters.
- Event matching uses nextest's emitted `crate::binary$test` prefix to avoid conflating duplicate test names across packages where possible.
- Suite-level JSON events are converted into readable runner messages.
- `--list-json` prints discovered tests and exits, which gives CI/tests a non-TUI entry point.
- Fixture coverage includes a small two-package workspace with duplicate test names.
