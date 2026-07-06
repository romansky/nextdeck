use crate::{
    config::AppSettings,
    output_pane::{SearchEditor, SearchEditorInput},
};

#[derive(Clone, Debug, Default)]
pub struct GlobalSettingsState {
    pub modal_open: bool,
    pub selected: SettingsField,
    pub editor_editing: bool,
    pub editor_draft: String,
    pub editor: SearchEditor,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SettingsField {
    #[default]
    Editor,
    TreeWidth,
    Theme,
    ColorBlindMode,
}

impl SettingsField {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::TreeWidth => "tests width",
            Self::Theme => "theme",
            Self::ColorBlindMode => "color-blind",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Editor => Self::TreeWidth,
            Self::TreeWidth => Self::Theme,
            Self::Theme => Self::ColorBlindMode,
            Self::ColorBlindMode => Self::Editor,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Editor => Self::ColorBlindMode,
            Self::TreeWidth => Self::Editor,
            Self::Theme => Self::TreeWidth,
            Self::ColorBlindMode => Self::Theme,
        }
    }
}

impl GlobalSettingsState {
    pub fn open(&mut self, settings: &AppSettings) {
        self.modal_open = true;
        self.editor_editing = false;
        self.sync_editor(settings);
    }

    pub fn close(&mut self) {
        self.modal_open = false;
        self.editor_editing = false;
    }

    pub fn sync_editor(&mut self, settings: &AppSettings) {
        let text = settings.editor_command.clone().unwrap_or_default();
        self.editor_draft = text.clone();
        self.editor.set_text(&text);
    }

    pub fn select_next(&mut self) {
        self.selected = self.selected.next();
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.previous();
    }

    pub fn begin_editor_edit(&mut self, settings: &AppSettings) {
        self.selected = SettingsField::Editor;
        self.editor_editing = true;
        self.sync_editor(settings);
    }

    pub fn edit_editor(&mut self, input: SearchEditorInput) {
        if self.editor.input(input) {
            self.editor_draft = self.editor.text();
        }
    }

    pub fn cancel_editor_edit(&mut self, settings: &AppSettings) {
        self.editor_editing = false;
        self.sync_editor(settings);
    }

    pub fn clear_editor_draft(&mut self) {
        self.editor_draft.clear();
        self.editor.clear();
    }
}
