# Xtask Integration

Nextdeck can discover repository automation built with the Rust
[`xtask`](https://github.com/matklad/cargo-xtask) convention. Press `X` to pick
a command, edit its arguments, run it, and read its output without leaving
the test UI.

Integration is opt-in. A project exposes a JSON description at:

```sh
cargo xtask nextdeck-info --format json
```

## Add It to a Clap Xtask

Most xtask repositories define this Cargo alias:

```toml
# .cargo/config.toml
[alias]
xtask = "run --package xtask --"
```

Enable the helper in the xtask crate:

```toml
# xtask/Cargo.toml
[dependencies]
nextdeck-test-events = { version = "0.1", features = ["xtask-clap"] }
```

Call `xtask_clap_info!` before `Cli::parse()`. The macro handles the synthetic
`nextdeck-info` command before Clap sees it.

```rust
use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, ValueEnum)]
enum Profile {
    Dev,
    Release,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Build release artifacts")]
    Release {
        #[arg(long)]
        allow_dirty: bool,

        #[arg(long, value_enum, default_value = "release")]
        profile: Profile,
    },
}

fn main() -> Result<()> {
    nextdeck_test_events::xtask_clap_info!(Cli);

    match Cli::parse().command {
        Command::Release {
            allow_dirty,
            profile,
        } => {
            // Run the existing task.
            let _ = (allow_dirty, profile);
            Ok(())
        }
    }
}
```

Check the endpoint without opening Nextdeck:

```sh
cargo xtask nextdeck-info --format json
```

## Supported Clap Arguments

The helper reads top-level Clap subcommands and exposes visible, named
arguments that Nextdeck can render:

| Clap shape | Nextdeck control |
| --- | --- |
| `bool` flag | on/off toggle |
| single value | text input |
| numeric default | numeric input |
| `ValueEnum` or possible values | finite choice |

Give integrated arguments long names such as `--allow-dirty`; these become the
flags used when Nextdeck runs the command. Help text and defaults come from the
Clap definition.

Positional, repeated, variadic, hidden, and other unsupported arguments are
left out of the generated metadata. They still work when the xtask is invoked
normally from the command line.

## Using It in Nextdeck

Press `X` to open the command picker and `Enter` to configure a command. The
left side contains its arguments and the right side contains live stdout and
stderr. Press `r` to run:

Here is a shortened capture of Nextdeck's own `tui-check` xtask:

```text
┌ Xtasks > tui-check ────────────────────────────────────────────────────────────────────────────────┐
│┌ Parameters ───────────────────────────┐ ┌ Output <#1-1/1> [s]nap-bottom:✓ ───────────────────────┐│
││@ --allow-dirty off # Allow cargo pa...│ │Run the selected xtask to see output here.              ││
││# bool: off, on (default: off)         │ │                                                        ││
││                                       │ │                                                        ││
││Run local TUI checks expected before...│ │                                                        ││
││cargo xtask tui-check                  │ │                                                        ││
││                                       │ │                                                        ││
││                                       │ │                                                        ││
│└───────────────────────────────────────┘ └ [/]search<[            ]> [o]pen-editor ───────────────┘│
└ [esc]back [tab]output [r]run ─────────────────────────────────────────────────────────────────────┘
```

```sh
cargo xtask <command> [args...]
```

Argument choices are remembered per project. Default-valued optional arguments
are omitted from the command line, and required values are checked before the
process starts.

Use the arrow keys to select and change arguments. `Enter` advances boolean and
enum values, or opens an editor for text and numeric values.

Press `Tab` to switch between parameters and output. Output has the same
search, filter, and external-open controls as test output. Press `Esc` to return
to the command picker.

## JSON Contract

Projects that do not use Clap can implement the endpoint directly. Version `1`
returns a command list with named arguments:

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
          "name": "profile",
          "long": "profile",
          "required": true,
          "value": {
            "type": "enum",
            "values": ["dev", "release"],
            "default": "release"
          }
        }
      ]
    }
  ]
}
```

Supported value types are `bool`, `string`, `number`, and `enum`. String,
number, and enum values are passed as `--name value`; a true boolean is passed
as `--flag`.
