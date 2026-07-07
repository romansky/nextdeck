# nextdeck-test-events

Tiny optional test-run event side channel for projects launched by NextDeck.

The crate writes JSONL only when `NEXTDECK_TEST_EVENTS` points at a file. In
normal test runs it is a no-op.

```rust
nextdeck_test_events::event!(
    level: nextdeck_test_events::Level::Info,
    target: "artifact-cache",
    "cache hit";
    "key" => cache_key,
    "source" => "local",
);
```

Each line is a schema-versioned generic event:

```json
{
  "schema_version": 1,
  "time": 1783420000000,
  "level": "info",
  "target": "artifact-cache",
  "message": "cache hit",
  "fields": { "key": "abc" },
  "source": {
    "module": "my_crate::tests",
    "file": "src/lib.rs",
    "line": 42
  }
}
```

## Clap xtask metadata

Enable `xtask-clap` to let an xtask expose NextDeck metadata from a normal Clap
command tree:

```toml
[dependencies]
nextdeck-test-events = { version = "0.1", features = ["xtask-clap"] }
```

```rust
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Check {
        #[arg(long)]
        allow_dirty: bool,
    },
}

fn main() -> Result<()> {
    nextdeck_test_events::xtask_clap_info!(Cli);

    match Cli::parse().command {
        Command::Check { allow_dirty } => {
            let _ = allow_dirty;
            Ok(())
        }
    }
}
```
