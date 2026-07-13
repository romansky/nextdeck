use crate::{
    config::{
        AppSettings, DEFAULT_OPEN_WITH_LABEL, DEFAULT_STORAGE_LOW_SPACE_THRESHOLD_GB,
        DEFAULT_TEST_OUTPUT_POLL_INTERVAL_MS, DEFAULT_TREE_WIDTH_PERCENT,
        MAX_STORAGE_LOW_SPACE_THRESHOLD_GB, MAX_TEST_OUTPUT_POLL_INTERVAL_MS,
        MAX_TREE_WIDTH_PERCENT, MIN_STORAGE_LOW_SPACE_THRESHOLD_GB,
        MIN_TEST_OUTPUT_POLL_INTERVAL_MS, MIN_TREE_WIDTH_PERCENT, ThemePreference,
        TreeDurationMode,
    },
    field_schema::ParameterDetails,
    input_field::{InputField, InputFieldInput},
};

pub(crate) const OPEN_WITH_PRESETS: &[Option<&str>] = &[
    None,
    Some("idea"),
    Some("code"),
    Some("cursor"),
    Some("zed"),
    Some("open"),
];

#[derive(Clone, Debug, Default)]
pub struct GlobalSettingsState {
    pub modal_open: bool,
    pub selected: SettingsField,
    pub open_with_editing: bool,
    pub open_with: InputField,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SettingsField {
    #[default]
    OpenWith,
    TreeWidth,
    TreeDuration,
    StorageThreshold,
    OutputPoll,
    Theme,
    ColorBlindMode,
}

impl SettingsField {
    pub const ALL: [Self; 7] = [
        Self::OpenWith,
        Self::TreeWidth,
        Self::TreeDuration,
        Self::StorageThreshold,
        Self::OutputPoll,
        Self::Theme,
        Self::ColorBlindMode,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenWith => "open with",
            Self::TreeWidth => "tests width",
            Self::TreeDuration => "tests time",
            Self::StorageThreshold => "low disk",
            Self::OutputPoll => "output poll",
            Self::Theme => "theme",
            Self::ColorBlindMode => "color-blind",
        }
    }

    pub(crate) fn details(self) -> ParameterDetails {
        match self {
            Self::OpenWith => ParameterDetails::string()
                .with_choices(
                    OPEN_WITH_PRESETS
                        .iter()
                        .map(|preset| preset.as_deref().unwrap_or(DEFAULT_OPEN_WITH_LABEL)),
                )
                .with_default(DEFAULT_OPEN_WITH_LABEL)
                .custom_value(),
            Self::TreeWidth => ParameterDetails::number()
                .with_choices([format!(
                    "{MIN_TREE_WIDTH_PERCENT}..{MAX_TREE_WIDTH_PERCENT}%"
                )])
                .with_default(format!("{DEFAULT_TREE_WIDTH_PERCENT}%")),
            Self::TreeDuration => {
                ParameterDetails::enum_values(TreeDurationMode::ALL.map(TreeDurationMode::label))
                    .with_default(TreeDurationMode::Wall.label())
            }
            Self::StorageThreshold => ParameterDetails::number()
                .with_choices([format!(
                    "{MIN_STORAGE_LOW_SPACE_THRESHOLD_GB}..{MAX_STORAGE_LOW_SPACE_THRESHOLD_GB} GiB"
                )])
                .with_default(format!("{DEFAULT_STORAGE_LOW_SPACE_THRESHOLD_GB} GiB")),
            Self::OutputPoll => ParameterDetails::number()
                .with_choices([format!(
                    "{MIN_TEST_OUTPUT_POLL_INTERVAL_MS}..{MAX_TEST_OUTPUT_POLL_INTERVAL_MS} ms"
                )])
                .with_default(format!("{DEFAULT_TEST_OUTPUT_POLL_INTERVAL_MS} ms")),
            Self::Theme => {
                ParameterDetails::enum_values(ThemePreference::ALL.map(ThemePreference::label))
                    .with_default(ThemePreference::Auto.label())
            }
            Self::ColorBlindMode => ParameterDetails::bool(false),
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::OpenWith => Self::TreeWidth,
            Self::TreeWidth => Self::TreeDuration,
            Self::TreeDuration => Self::StorageThreshold,
            Self::StorageThreshold => Self::OutputPoll,
            Self::OutputPoll => Self::Theme,
            Self::Theme => Self::ColorBlindMode,
            Self::ColorBlindMode => Self::OpenWith,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::OpenWith => Self::ColorBlindMode,
            Self::TreeWidth => Self::OpenWith,
            Self::TreeDuration => Self::TreeWidth,
            Self::StorageThreshold => Self::TreeDuration,
            Self::OutputPoll => Self::StorageThreshold,
            Self::Theme => Self::OutputPoll,
            Self::ColorBlindMode => Self::Theme,
        }
    }
}

impl GlobalSettingsState {
    pub fn open(&mut self, settings: &AppSettings) {
        self.modal_open = true;
        self.open_with_editing = false;
        self.sync_open_with(settings);
    }

    pub fn close(&mut self) {
        self.modal_open = false;
        self.open_with_editing = false;
    }

    pub fn sync_open_with(&mut self, settings: &AppSettings) {
        let text = settings.open_with_command.clone().unwrap_or_default();
        self.open_with.set_text(&text);
    }

    pub fn select_next(&mut self) {
        self.selected = self.selected.next();
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.previous();
    }

    pub fn begin_open_with_edit(&mut self, settings: &AppSettings) {
        self.selected = SettingsField::OpenWith;
        self.open_with_editing = true;
        self.sync_open_with(settings);
    }

    pub fn edit_open_with(&mut self, input: InputFieldInput) {
        self.open_with.input(input);
    }

    pub fn open_with_text(&self) -> String {
        self.open_with.text()
    }

    pub fn cancel_open_with_edit(&mut self, settings: &AppSettings) {
        self.open_with_editing = false;
        self.sync_open_with(settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_fields_describe_their_value_domains() {
        assert_eq!(
            SettingsField::OpenWith.details().render(),
            "# string: env/default, idea, code, cursor, zed, open (default: env/default; custom)"
        );
        assert_eq!(
            SettingsField::TreeWidth.details().render(),
            "# number: 25..70% (default: 45%)"
        );
        assert_eq!(
            SettingsField::TreeDuration.details().render(),
            "# enum: wall, aggregate (default: wall)"
        );
        assert_eq!(
            SettingsField::StorageThreshold.details().render(),
            "# number: 1..1024 GiB (default: 10 GiB)"
        );
        assert_eq!(
            SettingsField::OutputPoll.details().render(),
            "# number: 250..10000 ms (default: 1000 ms)"
        );
        assert_eq!(
            SettingsField::Theme.details().render(),
            "# enum: auto, dark, light (default: auto)"
        );
        assert_eq!(
            SettingsField::ColorBlindMode.details().render(),
            "# bool: off, on (default: off)"
        );
    }
}
