# nextdeck-test-events

Optional integrations for [Nextdeck](../README.md): test events and Clap-based
xtask discovery.

Test-event emission has no effect on ordinary test runs unless Nextdeck sets
`NEXTDECK_TEST_EVENTS`.

## Test events

```rust
let cache_key = "docs-v2";
nextdeck_test_events::event!(
    level: nextdeck_test_events::Level::Info,
    target: "artifact-cache",
    "cache hit";
    "key" => cache_key,
);
```

See the [test events guide](../docs/test-events/README.md) for setup, event
fields, and how events appear in Nextdeck.

## Clap xtask metadata

Enable the `xtask-clap` feature in an xtask crate:

```toml
[dependencies]
nextdeck-test-events = { version = "0.1", features = ["xtask-clap"] }
```

Then call the metadata handler before parsing the CLI:

```rust
fn main() -> anyhow::Result<()> {
    nextdeck_test_events::xtask_clap_info!(Cli);
    let cli = Cli::parse();
    // Dispatch the existing command.
    Ok(())
}
```

See the [xtask integration guide](../docs/xtask-integration/README.md) for a
complete Clap example and the JSON contract.
