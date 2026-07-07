# Xtask Integration

Nextdeck can surface project automation from the common Rust `xtask` convention.
Projects opt in by exposing a small JSON description at:

```sh
cargo xtask nextdeck-info --format json
```

Most projects make `cargo xtask` available with a Cargo alias:

```toml
# .cargo/config.toml
[alias]
xtask = "run --package xtask --"
```

## Contract

The endpoint prints a JSON AST. Version `1` supports commands with named
parameters that Nextdeck can render as a modal form.

```json
{
  "schema_version": 1,
  "commands": [
    {
      "name": "release",
      "about": "Build release artifacts",
      "args": [
        {
          "name": "allow-dirty",
          "long": "allow-dirty",
          "help": "Allow a dirty worktree",
          "value": { "type": "bool", "default": false }
        },
        {
          "name": "version",
          "long": "version",
          "required": true,
          "help": "Release version",
          "value": { "type": "string" }
        },
        {
          "name": "retries",
          "long": "retries",
          "help": "Retry count",
          "value": { "type": "number", "default": 1 }
        },
        {
          "name": "profile",
          "long": "profile",
          "help": "Build profile",
          "value": {
            "type": "enum",
            "values": ["dev", "release"],
            "default": "dev"
          }
        }
      ]
    }
  ]
}
```

Supported value types:

- `bool`: rendered as an on/off toggle and passed as `--flag` when true.
- `string`: rendered as a text input and passed as `--name value`.
- `number`: rendered as a numeric input and passed as `--name value`.
- `enum`: rendered as a finite set and passed as `--name value`.

Optional values that still match their default are omitted from the command
line. Required values are validated before running.

## Clap-Friendly Integration

The helper crate can generate this metadata from a normal Clap command tree.
Call the macro before `Cli::parse()` so `cargo xtask nextdeck-info --format json`
is handled before Clap rejects the synthetic command.

```rust
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Build release artifacts")]
    Release {
        #[arg(long)]
        allow_dirty: bool,
        #[arg(long)]
        version: Option<String>,
    },
}

fn main() -> Result<()> {
    nextdeck_test_events::xtask_clap_info!(Cli);

    match Cli::parse().command {
        Command::Release { allow_dirty, version } => {
            // Existing release implementation.
            let _ = (allow_dirty, version);
            Ok(())
        }
    }
}
```

Cargo dependency:

```toml
[dependencies]
nextdeck-test-events = { version = "0.1", features = ["xtask-clap"] }
```

The Clap helper covers the simple shapes Nextdeck can render today: named
booleans, single string values, numeric values with numeric defaults, and enums
from `ValueEnum` or possible values. Positional, repeated, and variadic args are
intentionally omitted from the generated metadata.

## Nextdeck UI

Press `x` to open the xtasks modal. Nextdeck discovers tasks from the active
project directory and shows a command picker. Press `enter` on a command to open
a command frame inside the same modal; the title changes to a breadcrumb such as
`Xtasks > release`, with a back button returning to the picker. The command frame
shows parameters on the left and command output on the right. From there, edit
named parameters and run:

```sh
cargo xtask <command> [args...]
```
