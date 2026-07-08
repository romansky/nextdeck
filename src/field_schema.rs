pub(crate) fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParameterDetails {
    kind: ParameterKind,
    choices: Vec<String>,
    default: Option<String>,
    custom_value: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParameterKind {
    Bool,
    Enum,
    Number,
    String,
}

impl ParameterDetails {
    pub(crate) fn bool(default: bool) -> Self {
        Self {
            kind: ParameterKind::Bool,
            choices: vec!["off".to_owned(), "on".to_owned()],
            default: Some(on_off(default).to_owned()),
            custom_value: false,
        }
    }

    pub(crate) fn enum_values(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::new(ParameterKind::Enum).with_choices(values)
    }

    pub(crate) fn number() -> Self {
        Self::new(ParameterKind::Number)
    }

    pub(crate) fn string() -> Self {
        Self::new(ParameterKind::String)
    }

    pub(crate) fn with_choices(
        mut self,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.choices = values
            .into_iter()
            .map(Into::into)
            .filter(|value: &String| !value.trim().is_empty())
            .collect();
        self
    }

    pub(crate) fn with_default(mut self, default: impl Into<String>) -> Self {
        let default = default.into();
        if !default.trim().is_empty() {
            self.default = Some(default);
        }
        self
    }

    pub(crate) fn custom_value(mut self) -> Self {
        self.custom_value = true;
        self
    }

    pub(crate) fn render(&self) -> String {
        let mut details = format!("# {}", self.kind.label());
        if !self.choices.is_empty() {
            details.push_str(": ");
            details.push_str(&self.choices.join(", "));
        }

        let mut notes = Vec::new();
        if let Some(default) = &self.default {
            notes.push(format!("default: {default}"));
        }
        if self.custom_value {
            notes.push("[e] custom".to_owned());
        }
        if !notes.is_empty() {
            details.push_str(" (");
            details.push_str(&notes.join("; "));
            details.push(')');
        }

        details
    }

    fn new(kind: ParameterKind) -> Self {
        Self {
            kind,
            choices: Vec::new(),
            default: None,
            custom_value: false,
        }
    }
}

impl ParameterKind {
    fn label(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Enum => "enum",
            Self::Number => "number",
            Self::String => "string",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_parameter_details_with_one_convention() {
        assert_eq!(
            ParameterDetails::bool(false).render(),
            "# bool: off, on (default: off)"
        );
        assert_eq!(
            ParameterDetails::number()
                .with_choices(["profile", "0..20"])
                .with_default("profile")
                .custom_value()
                .render(),
            "# number: profile, 0..20 (default: profile; [e] custom)"
        );
    }
}
