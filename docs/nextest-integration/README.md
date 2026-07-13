# Nextest Integration

Nextdeck wraps `cargo-nextest`; it is not a separate test runner. Test
discovery, filtering, execution, retries, and profiles remain nextest concepts;
Nextdeck puts them in an interactive terminal UI.

## Requirements

Install both commands and make sure they are on `PATH`:

```sh
cargo install --locked cargo-nextest
cargo install --locked nextdeck
```

From a Cargo workspace, start Nextdeck with:

```sh
nextdeck
```

At startup, Nextdeck calls `cargo nextest list --message-format json` and builds
a tree from nextest's package, target, and test metadata.

## Selecting What to Run

Use the tree to select a workspace, package, test binary, module, or individual
test, then press `r`. Nextdeck translates the selection into the corresponding
nextest arguments.

After a run, `j` and `J` move between failures. A custom run can also use the
failed tests from the previous run as its scope.

Press `R` to configure options before running. The form currently covers:

- selected, workspace, or failed-test scope
- nextest profile
- configured or custom filterset
- ignored-test mode
- retries and flaky-test result
- fail-fast and maximum failures
- output capture
- debugger command for one selected test
- stress count or duration for one selected test

Use the up and down arrows to select a field. Left and right change boolean and
enum values; `Enter` advances those values or opens an editor for fields that
accept text or numbers.

Nextdeck reads profile names and default filtersets from
`.config/nextest.toml`. The file remains owned by nextest and continues to
control normal nextest behavior.

## Other Workspaces

Use `--working-dir` to open another Cargo workspace:

```sh
nextdeck --working-dir ../my-workspace
```

The directory may be the workspace root or any directory inside it. Nextdeck
resolves the workspace root and uses it for Nextest, xtasks, Git status,
storage, and saved xtask values.

## Output and Source Files

Nextdeck keeps captured output with the test that produced it. Select a test
and press `Enter` to open its result, or focus the output pane and press `/`
to search it. Press `o` to open a selected source file or the current output.

The opener is resolved in this order:

1. `--open-with`
2. the command saved in Nextdeck settings
3. `NEXTDECK_EDITOR`
4. `VISUAL`
5. `EDITOR`

Use `?` inside any view for its complete key list.
