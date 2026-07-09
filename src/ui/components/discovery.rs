use ratatui::{Frame, text::Line};

use crate::{app::App, theme::Theme};

use super::super::{
    geometry::centered_rect,
    primitives::{ModalChrome, draw_modal_lines, draw_modal_output_lines},
};
use super::output::OutputPanel;

pub(in crate::ui) struct DiscoveryModal<'a> {
    app: &'a App,
}

impl<'a> DiscoveryModal<'a> {
    pub(in crate::ui) fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme) {
        let area = centered_rect(62, 58, frame.area());
        if self.app.discovery.error.is_some() {
            let output = OutputPanel::new(
                &self.app.main_output,
                self.app.output_source_text(),
                "Discovery",
                false,
            )
            .content(theme);
            let actions = Self::error_actions(&output.actions);
            draw_modal_output_lines(
                frame,
                theme,
                area,
                ModalChrome {
                    title: &output.status,
                    actions: Some(&actions),
                },
                output.lines,
                output.viewport,
            );
        } else {
            let lines = vec![
                Line::styled(
                    format!("{} Discovering tests", self.app.discovery_spinner()),
                    theme.accent(),
                ),
                Line::from(""),
                Line::styled(
                    "Running cargo nextest list --message-format json",
                    theme.text(),
                ),
                Line::styled(
                    "Cold discovery may compile test binaries first.",
                    theme.muted(),
                ),
                Line::styled(
                    format!("Elapsed: {}s", self.app.discovery_elapsed_seconds()),
                    theme.text(),
                ),
            ];
            draw_modal_lines(
                frame,
                theme,
                area,
                ModalChrome {
                    title: "Discovery",
                    actions: Some("[q]quit"),
                },
                lines,
            );
        }
    }

    pub(in crate::ui) fn error_actions(output_actions: &str) -> String {
        format!("[u]retry {output_actions} [q]quit")
    }
}
