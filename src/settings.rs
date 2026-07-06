use crate::{
    config::AppSettings,
    input_field::{InputField, InputFieldInput},
};

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
    StorageThreshold,
    Theme,
    ColorBlindMode,
}

impl SettingsField {
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenWith => "open with",
            Self::TreeWidth => "tests width",
            Self::StorageThreshold => "low disk",
            Self::Theme => "theme",
            Self::ColorBlindMode => "color-blind",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::OpenWith => Self::TreeWidth,
            Self::TreeWidth => Self::StorageThreshold,
            Self::StorageThreshold => Self::Theme,
            Self::Theme => Self::ColorBlindMode,
            Self::ColorBlindMode => Self::OpenWith,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::OpenWith => Self::ColorBlindMode,
            Self::TreeWidth => Self::OpenWith,
            Self::StorageThreshold => Self::TreeWidth,
            Self::Theme => Self::StorageThreshold,
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

    pub fn clear_open_with_draft(&mut self) {
        self.open_with.clear();
    }
}
