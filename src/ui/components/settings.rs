use ratatui::{
    Frame,
    layout::Alignment,
    widgets::{Paragraph, Wrap},
};

use crate::{
    app::App,
    field_schema::on_off,
    parameter_list::{ParameterList, ParameterListRow},
    settings::SettingsField,
    theme::Theme,
};

use super::super::{
    geometry::centered_rect,
    primitives::{ModalChrome, draw_modal_shell},
    view_helpers::{SELECTABLE_FIELD_PREFIX_WIDTH, fit_line_content, parameter_list_styles},
};

const OPEN_WITH_VALUE_WIDTH: usize = 42;

pub(in crate::ui) struct SettingsModal<'a> {
    app: &'a App,
}

impl<'a> SettingsModal<'a> {
    pub(in crate::ui) const FIELD_LABEL_WIDTH: usize = 13;

    pub(in crate::ui) fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme) {
        let area = centered_rect(72, 62, frame.area());
        let inner = draw_modal_shell(
            frame,
            theme,
            area,
            ModalChrome {
                title: "Settings",
                actions: Some(Self::actions(self.app)),
            },
        );
        let content_width = inner.width as usize;
        let lines = ParameterList::new(
            &Self::rows(self.app),
            SELECTABLE_FIELD_PREFIX_WIDTH,
            Self::FIELD_LABEL_WIDTH,
            content_width,
            parameter_list_styles(theme),
        )
        .render();
        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Left)
            .style(theme.text())
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, inner);
    }

    pub(in crate::ui) fn actions(app: &App) -> &'static str {
        if app.global_settings.open_with_editing {
            "[esc]cancel"
        } else {
            "[esc]close"
        }
    }

    pub(in crate::ui) fn rows(app: &App) -> Vec<ParameterListRow> {
        SettingsField::ALL
            .into_iter()
            .map(|field| {
                let selected = app.global_settings.selected == field;
                ParameterListRow {
                    selected,
                    active: selected,
                    label: field.label().to_owned(),
                    value: Self::value(app, field),
                    details: Some(field.details()),
                    ..Default::default()
                }
            })
            .collect()
    }

    pub(in crate::ui) fn value(app: &App, field: SettingsField) -> String {
        match field {
            SettingsField::OpenWith if app.global_settings.open_with_editing => {
                format!(
                    "[{}]",
                    app.global_settings
                        .open_with
                        .view(OPEN_WITH_VALUE_WIDTH, true)
                )
            }
            SettingsField::OpenWith => {
                format!(
                    "[{}]",
                    fit_line_content(app.settings.open_with_label(), OPEN_WITH_VALUE_WIDTH)
                )
            }
            SettingsField::TreeWidth => format!("{}%", app.settings.tree_width_percent),
            SettingsField::TreeDuration => app.settings.tree_duration_mode.label().to_owned(),
            SettingsField::StorageThreshold => {
                format!("{} GiB", app.settings.storage_low_space_threshold_gb)
            }
            SettingsField::OutputPoll => {
                format!("{} ms", app.settings.test_output_poll_interval_ms)
            }
            SettingsField::Theme => app.settings.theme_mode.label().to_owned(),
            SettingsField::ColorBlindMode => on_off(app.settings.color_blind_mode).to_owned(),
        }
    }
}
