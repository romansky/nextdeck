use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{XtaskArgDefaultExt, XtaskManifest, XtaskValueSpec};
use crate::field_schema::on_off;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub(super) enum XtaskArgValue {
    Bool(bool),
    Number(String),
    String(String),
    Enum(String),
}

impl XtaskArgValue {
    pub(super) fn display(&self) -> String {
        match self {
            Self::Bool(value) => on_off(*value).to_owned(),
            Self::Number(value) | Self::String(value) | Self::Enum(value) => value.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(super) struct XtaskPreferences {
    pub(super) values: BTreeMap<String, BTreeMap<String, XtaskArgValue>>,
}

impl XtaskPreferences {
    pub(super) fn reconcile(&mut self, manifest: &XtaskManifest) {
        let previous = std::mem::take(&mut self.values);
        self.values = manifest
            .commands
            .iter()
            .map(|command| {
                let previous_args = previous.get(&command.name);
                let args = command
                    .args
                    .iter()
                    .map(|arg| {
                        let value = previous_args
                            .and_then(|args| args.get(&arg.name))
                            .filter(|value| value.matches(&arg.value))
                            .cloned()
                            .unwrap_or_else(|| arg.default_value());
                        (arg.name.clone(), value)
                    })
                    .collect();
                (command.name.clone(), args)
            })
            .collect();
    }

    pub(super) fn overrides_for(&self, manifest: &XtaskManifest) -> Self {
        let values = manifest
            .commands
            .iter()
            .filter_map(|command| {
                let args = command
                    .args
                    .iter()
                    .filter_map(|arg| {
                        let value = self.value(&command.name, &arg.name)?;
                        (value != &arg.default_value()).then(|| (arg.name.clone(), value.clone()))
                    })
                    .collect::<BTreeMap<_, _>>();
                (!args.is_empty()).then(|| (command.name.clone(), args))
            })
            .collect();
        Self { values }
    }

    pub(super) fn value(&self, command: &str, arg: &str) -> Option<&XtaskArgValue> {
        self.values.get(command)?.get(arg)
    }

    pub(super) fn value_mut(&mut self, command: &str, arg: &str) -> Option<&mut XtaskArgValue> {
        self.values.get_mut(command)?.get_mut(arg)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl XtaskArgValue {
    fn matches(&self, spec: &XtaskValueSpec) -> bool {
        match (self, spec) {
            (Self::Bool(_), XtaskValueSpec::Bool { .. }) => true,
            (Self::Number(value), XtaskValueSpec::Number { .. }) => {
                value.trim().is_empty() || value.parse::<i64>().is_ok()
            }
            (Self::String(_), XtaskValueSpec::String { .. }) => true,
            (Self::Enum(value), XtaskValueSpec::Enum { values, .. }) => {
                values.iter().any(|allowed| allowed == value)
            }
            _ => false,
        }
    }
}
