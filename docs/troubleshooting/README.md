# Troubleshooting

## Tests Are Missing or Do Not Start

Run Nextest from the same Cargo workspace:

```sh
cargo nextest run
```

Resolve any Cargo or Nextest error first. Also confirm that `cargo-nextest` is
available on `PATH` and that Nextdeck was opened from the intended workspace.

## Xtasks Are Missing

Run the discovery endpoint from the workspace root:

```sh
cargo xtask nextdeck-info --format json
```

It must exit successfully and write only the manifest to stdout. See the
[xtask integration guide](../xtask-integration/README.md) for setup and the
supported format.

## Test Events Are Missing

The event helper emits only for tests launched by Nextdeck. A normal
`cargo test` or `cargo nextest run` intentionally produces no event frames.

Confirm that the helper dependency and event macro follow the
[test-events guide](../test-events/README.md).

## Sources or Output Do Not Open

Set the opener in Nextdeck's settings or provide `NEXTDECK_EDITOR`, `VISUAL`,
or `EDITOR`. Commands that need placeholders can use `{file}` and `{line}`.

## Diagnostic Log

Start Nextdeck with diagnostic logging enabled:

```sh
nextdeck --debug
```

Logs are appended to `~/.nextdeck/debug.log`. Before sharing the file, remove
workspace paths, source contents, command arguments, or other sensitive data.

When opening a troubleshooting issue, include the Nextdeck version, operating
system, the failing workspace command, and the smallest relevant log excerpt.

## Source References

- [Diagnostic logging](../../src/main.rs)
- [Editor resolution](../../src/editor.rs)
