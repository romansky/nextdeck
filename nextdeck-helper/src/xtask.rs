#[cfg(feature = "xtask-clap")]
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};

#[cfg(feature = "xtask-clap")]
use clap::{Arg, ArgAction, Command, CommandFactory};
use serde::{Deserialize, Serialize};

pub const INFO_COMMAND: &str = "nextdeck-info";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Json(serde_json::Error),
    UnsupportedFormat(String),
    MissingFormatValue,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::UnsupportedFormat(format) => {
                write!(formatter, "unsupported Nextdeck info format: {format}")
            }
            Self::MissingFormatValue => write!(formatter, "--format requires a value"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct XtaskManifest {
    pub schema_version: u32,
    pub commands: Vec<XtaskCommand>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct XtaskCommand {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<XtaskArg>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct XtaskArg {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<char>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub required: bool,
    pub value: XtaskValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum XtaskValue {
    Bool {
        #[serde(default)]
        default: bool,
    },
    Number {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<i64>,
    },
    String {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
    },
    Enum {
        values: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
    },
}

#[cfg(feature = "xtask-clap")]
pub fn handle_nextdeck_info<T>() -> Result<bool>
where
    T: CommandFactory,
{
    handle_nextdeck_info_from::<T, _, _, _>(std::env::args_os(), io::stdout())
}

#[cfg(feature = "xtask-clap")]
pub fn handle_nextdeck_info_from<T, I, S, W>(args: I, writer: W) -> Result<bool>
where
    T: CommandFactory,
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
    W: Write,
{
    let mut args = args.into_iter().map(Into::into);
    let _binary = args.next();
    let Some(command) = args.next() else {
        return Ok(false);
    };
    if command != INFO_COMMAND {
        return Ok(false);
    }

    let mut format = "json".to_owned();
    while let Some(arg) = args.next() {
        if arg == "--format" {
            let Some(value) = args.next() else {
                return Err(Error::MissingFormatValue);
            };
            format = value.to_string_lossy().into_owned();
        } else if let Some(value) = arg
            .to_string_lossy()
            .strip_prefix("--format=")
            .map(ToOwned::to_owned)
        {
            format = value;
        } else {
            return Err(Error::UnsupportedFormat(arg.to_string_lossy().into_owned()));
        }
    }

    if format != "json" {
        return Err(Error::UnsupportedFormat(format));
    }

    write_manifest_for::<T, W>(writer)?;
    Ok(true)
}

#[cfg(feature = "xtask-clap")]
pub fn write_manifest_for<T, W>(writer: W) -> Result<()>
where
    T: CommandFactory,
    W: Write,
{
    write_manifest(&command_manifest(T::command()), writer)
}

pub fn write_manifest<W>(manifest: &XtaskManifest, mut writer: W) -> Result<()>
where
    W: Write,
{
    serde_json::to_writer_pretty(&mut writer, manifest)?;
    writeln!(writer)?;
    Ok(())
}

#[cfg(feature = "xtask-clap")]
pub fn command_manifest(mut command: Command) -> XtaskManifest {
    command.build();
    XtaskManifest {
        schema_version: SCHEMA_VERSION,
        commands: command
            .get_subcommands()
            .filter(|command| !command.is_hide_set())
            .filter(|command| command.get_name() != INFO_COMMAND)
            .filter(|command| command.get_name() != "help")
            .map(command_spec)
            .collect(),
    }
}

#[cfg(feature = "xtask-clap")]
fn command_spec(command: &Command) -> XtaskCommand {
    XtaskCommand {
        name: command.get_name().to_owned(),
        about: command
            .get_about()
            .or_else(|| command.get_long_about())
            .map(ToString::to_string),
        args: command
            .get_arguments()
            .filter(|arg| !arg.is_hide_set())
            .filter(|arg| !arg.is_positional())
            .filter(|arg| arg.get_long().is_some() || arg.get_short().is_some())
            .filter_map(arg_spec)
            .collect(),
    }
}

#[cfg(feature = "xtask-clap")]
fn arg_spec(arg: &Arg) -> Option<XtaskArg> {
    let value = arg_value(arg)?;
    Some(XtaskArg {
        name: arg.get_id().as_str().to_owned(),
        long: arg.get_long().map(ToOwned::to_owned),
        short: arg.get_short(),
        help: arg.get_help().map(ToString::to_string),
        required: arg.is_required_set(),
        value,
    })
}

#[cfg(feature = "xtask-clap")]
fn arg_value(arg: &Arg) -> Option<XtaskValue> {
    match arg.get_action() {
        ArgAction::SetTrue => Some(XtaskValue::Bool { default: false }),
        ArgAction::SetFalse => Some(XtaskValue::Bool { default: true }),
        ArgAction::Set => {
            let defaults = default_values(arg);
            let possible_values = arg
                .get_value_parser()
                .possible_values()
                .map(|values| {
                    values
                        .filter(|value| !value.is_hide_set())
                        .map(|value| value.get_name().to_owned())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !possible_values.is_empty() {
                return Some(XtaskValue::Enum {
                    values: possible_values,
                    default: defaults.first().cloned(),
                });
            }
            if let Some(default) = defaults.first().and_then(|value| value.parse().ok()) {
                return Some(XtaskValue::Number {
                    default: Some(default),
                });
            }
            Some(XtaskValue::String {
                default: defaults.first().cloned(),
            })
        }
        _ => None,
    }
}

#[cfg(feature = "xtask-clap")]
fn default_values(arg: &Arg) -> Vec<String> {
    arg.get_default_values()
        .iter()
        .filter_map(|value| value.to_str().map(ToOwned::to_owned))
        .collect()
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(all(test, feature = "xtask-clap"))]
mod tests {
    use clap::{Parser, Subcommand, ValueEnum};

    use super::*;

    #[derive(Parser)]
    struct ExampleCli {
        #[command(subcommand)]
        command: ExampleCommand,
    }

    #[derive(Subcommand)]
    enum ExampleCommand {
        #[command(about = "Run checks")]
        Check {
            #[arg(long, help = "Allow a dirty worktree")]
            allow_dirty: bool,
            #[arg(long, default_value_t = 2)]
            retries: i64,
            #[arg(long, value_enum, default_value_t = Profile::Dev)]
            profile: Profile,
            #[arg(long)]
            name: Option<String>,
            #[arg(long, action = clap::ArgAction::SetFalse)]
            color: bool,
        },
    }

    #[derive(Clone, Debug, ValueEnum)]
    enum Profile {
        Dev,
        Release,
    }

    #[test]
    fn generates_nextdeck_manifest_from_clap() {
        let manifest = command_manifest(ExampleCli::command());

        assert_eq!(manifest.schema_version, SCHEMA_VERSION);
        assert_eq!(manifest.commands[0].name, "check");
        assert_eq!(manifest.commands[0].args[0].name, "allow_dirty");
        assert_eq!(
            manifest.commands[0].args[0].long.as_deref(),
            Some("allow-dirty")
        );
        assert!(matches!(
            manifest.commands[0].args[0].value,
            XtaskValue::Bool { default: false }
        ));
        assert!(matches!(
            manifest.commands[0].args[1].value,
            XtaskValue::Number { default: Some(2) }
        ));
        assert!(matches!(
            &manifest.commands[0].args[2].value,
            XtaskValue::Enum { values, default }
                if values == &vec!["dev".to_owned(), "release".to_owned()]
                    && default.as_deref() == Some("dev")
        ));
        assert!(manifest.commands[0].args.iter().any(|arg| {
            arg.name == "color" && matches!(arg.value, XtaskValue::Bool { default: true })
        }));
    }

    #[test]
    fn handles_info_request_before_clap_parse() {
        let mut output = Vec::new();
        let handled = handle_nextdeck_info_from::<ExampleCli, _, _, _>(
            ["xtask", INFO_COMMAND, "--format", "json"],
            &mut output,
        )
        .expect("info");

        assert!(handled);
        let json = String::from_utf8(output).expect("utf8");
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"name\": \"check\""));
        assert!(json.contains("\"long\": \"allow-dirty\""));
    }
}
