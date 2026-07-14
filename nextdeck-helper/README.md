# nextdeck-helper

Rust helper lib for test-event and Clap xtask integrations.

## Test Events

```toml
[dev-dependencies]
nextdeck-helper = "0.1"
```

```rust
nextdeck_helper::event!(
    level: nextdeck_helper::Level::Info,
    target: "artifact-cache",
    "cache hit";
    "key" => "docs-v2",
);
```

Events are emitted only when `NEXTDECK_TEST_EVENTS` environment variable is set (Nextdeck does this automatically).
See [test-events guide](https://docs.rs/crate/nextdeck/latest/source/docs/test-events/README.md) for more info.

## Clap Xtask Metadata

Enable the opt-in feature in the xtask crate:

```toml
[dependencies]
nextdeck-helper = { version = "0.1", features = ["xtask-clap"] }
```

Handle Nextdeck's metadata request before parsing the normal command line:

```rust
use clap::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    nextdeck_helper::xtask_clap_info!(Cli);
    let cli = Cli::parse();
    // Dispatch the existing command.
    Ok(())
}
```

See
[xtask integration guide](https://docs.rs/crate/nextdeck/latest/source/docs/xtask-integration/README.md)
for more info.
