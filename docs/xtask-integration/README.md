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

## Clap-Friendly Snippet

This keeps the integration explicit and stable while the actual task
implementation can continue using Clap normally.

```rust
use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Print nextdeck xtask integration metadata")]
    NextdeckInfo {
        #[arg(long, value_enum, default_value_t = InfoFormat::Json)]
        format: InfoFormat,
    },
    #[command(about = "Build release artifacts")]
    Release {
        #[arg(long)]
        allow_dirty: bool,
        #[arg(long)]
        version: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum InfoFormat {
    Json,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::NextdeckInfo { format: InfoFormat::Json } => {
            let manifest = serde_json::json!({
                "schema_version": 1,
                "commands": [{
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
                            "help": "Release version",
                            "value": { "type": "string" }
                        }
                    ]
                }]
            });
            serde_json::to_writer_pretty(std::io::stdout(), &manifest)?;
            println!();
            Ok(())
        }
        Command::Release { allow_dirty, version } => {
            // Existing release implementation.
            let _ = (allow_dirty, version);
            Ok(())
        }
    }
}
```

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
