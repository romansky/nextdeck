use ratatui::{
    Frame,
    text::{Line, Span},
};

use crate::{app::App, disk_usage::format_bytes, theme::Theme};

use super::super::{
    geometry::centered_rect,
    primitives::{ModalChrome, draw_modal_lines},
};

pub(in crate::ui) struct DiskCleanupModal<'a> {
    app: &'a App,
}

impl<'a> DiskCleanupModal<'a> {
    pub(in crate::ui) fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme) {
        let area = centered_rect(70, 62, frame.area());
        draw_modal_lines(
            frame,
            theme,
            area,
            ModalChrome {
                title: "Disk Cleanup",
                actions: Some(Self::actions()),
            },
            Self::lines(self.app, theme),
        );
    }

    pub(in crate::ui) fn actions() -> &'static str {
        "[c]cargo-clean [r]refresh [esc]close"
    }

    pub(in crate::ui) fn lines(app: &App, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = vec![
            Line::styled("Disk Usage", theme.title(false)),
            Line::from(""),
        ];

        if let Some(snapshot) = &app.disk_usage.snapshot {
            for entry in &snapshot.entries {
                lines.push(Line::from(vec![
                    Span::styled(format!("{:<15}", entry.label), theme.muted()),
                    Span::styled(format!("{:>10}", format_bytes(entry.bytes)), theme.text()),
                    Span::raw("  "),
                    Span::styled(entry.path.display().to_string(), theme.muted()),
                ]));
            }
        } else if app.disk_usage.loading {
            lines.push(Line::styled("Scanning disk usage...", theme.muted()));
        } else if let Some(error) = &app.disk_usage.error {
            lines.push(Line::styled(error.clone(), theme.danger()));
        } else {
            lines.push(Line::styled("No disk usage snapshot yet.", theme.muted()));
        }

        lines.extend([
            Line::from(""),
            Line::styled("Cleanup", theme.title(false)),
            Line::styled(
                "cargo clean removes this workspace's target directory.",
                theme.text(),
            ),
        ]);

        if app.disk_cleanup.running {
            lines.push(Line::from(""));
            lines.push(Line::styled("cargo clean running...", theme.accent()));
        }

        if let Some(result) = &app.disk_cleanup.last_result {
            lines.push(Line::from(""));
            match result {
                Ok(()) => lines.push(Line::styled("Last cleanup completed.", theme.success())),
                Err(error) => lines.push(Line::styled(
                    format!("Last cleanup failed: {error}"),
                    theme.danger(),
                )),
            }
        }

        lines
    }
}
