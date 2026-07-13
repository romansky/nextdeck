use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Clear, Paragraph, Wrap},
};

use crate::{
    app::App,
    parameter_list::{ParameterList, ParameterListRow},
    theme::Theme,
    xtask::{XtaskArgSpecExt, XtaskCommandSpec, XtaskDetailFocus, XtaskState, XtaskValueSpecExt},
};

use super::super::{
    geometry::centered_rect,
    primitives::{
        AutoColumn, AutoColumnLayout, ModalChrome, draw_modal_shell, scrollable_paragraph,
    },
    view_helpers::{
        SELECTABLE_FIELD_PREFIX_WIDTH, fit_line_prefix, parameter_list_styles,
        parameter_value_width,
    },
};
use super::output::OutputPanel;

const COMMAND_NAME_MAX_WIDTH: usize = 30;
const FIELD_HINT_MIN_COLUMN_WIDTH: usize = 6;
const PARAM_LABEL_MAX_WIDTH: usize = 24;
const PARAM_LABEL_MIN_WIDTH: usize = 6;

pub(in crate::ui) struct XtasksModal<'a> {
    app: &'a App,
}

struct XtaskDetailAreas {
    parameters: Rect,
    separator: Rect,
    output: Rect,
}

impl<'a> XtasksModal<'a> {
    pub(in crate::ui) fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme) {
        let area = centered_rect(88, 82, frame.area());
        let title = self.title();
        let inner = draw_modal_shell(
            frame,
            theme,
            area,
            ModalChrome {
                title: &title,
                actions: Some(Self::actions(&self.app.xtasks)),
            },
        );

        if self.app.xtasks.detail_open {
            self.render_detail(frame, theme, inner);
        } else {
            self.render_command_picker(frame, theme, inner);
        }
    }

    pub(in crate::ui) fn detail_parameters_area(area: Rect) -> Rect {
        Self::detail_areas(area).parameters
    }

    pub(in crate::ui) fn detail_output_area(area: Rect) -> Rect {
        Self::detail_areas(area).output
    }

    pub(in crate::ui) fn command_lines(
        xtasks: &XtaskState,
        theme: &Theme,
        content_width: usize,
    ) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        if xtasks.loading {
            lines.push(Line::styled(
                "Loading cargo xtask metadata...",
                theme.muted(),
            ));
            if xtasks.manifest.is_none() {
                return lines;
            }
        }
        if let Some(error) = &xtasks.error {
            lines.push(Line::styled(format!("Error: {error}"), theme.danger()));
        }
        let Some(manifest) = &xtasks.manifest else {
            lines.extend([
                Line::from(""),
                Line::styled(
                    "This project has not exposed nextdeck xtask metadata yet.",
                    theme.text(),
                ),
                Line::styled(
                    "Expected: cargo xtask nextdeck-info --format json",
                    theme.muted(),
                ),
            ]);
            return lines;
        };
        if manifest.commands.is_empty() {
            lines.push(Line::styled("No xtask commands exposed.", theme.muted()));
            return lines;
        }

        let comments = manifest
            .commands
            .iter()
            .map(|command| {
                command
                    .about
                    .as_deref()
                    .map(str::trim)
                    .filter(|about| !about.is_empty())
                    .map(|about| format!("# {about}"))
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let rows = manifest
            .commands
            .iter()
            .zip(&comments)
            .enumerate()
            .map(|(index, (command, comment))| {
                vec![
                    if index == xtasks.selected_command {
                        ">"
                    } else {
                        " "
                    },
                    command.name.as_str(),
                    comment.as_str(),
                ]
            })
            .collect::<Vec<_>>();
        let layout = AutoColumnLayout::compute(
            &[
                AutoColumn { max_width: Some(1) },
                AutoColumn {
                    max_width: Some(COMMAND_NAME_MAX_WIDTH),
                },
                AutoColumn { max_width: None },
            ],
            &rows,
            content_width,
        );

        for (index, command) in manifest.commands.iter().enumerate() {
            let selected = index == xtasks.selected_command;
            let style = if selected {
                theme.selected()
            } else {
                theme.text()
            };
            let comment = comments.get(index).map(String::as_str).unwrap_or("");
            let marker = if selected { ">" } else { " " };
            lines.push(layout.row(&[
                (marker, style),
                (command.name.as_str(), style),
                (comment, theme.muted()),
            ]));
        }

        lines
    }

    pub(in crate::ui) fn parameter_lines(
        xtasks: &XtaskState,
        theme: &Theme,
        content_width: usize,
        focused: bool,
    ) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        if let Some(command) = xtasks.selected_command() {
            if command.args.is_empty() {
                lines.push(Line::styled("No parameters.", theme.muted()));
            } else {
                let label_width = Self::param_label_width(command, content_width);
                let value_width = parameter_value_width(content_width, label_width);
                let rows = Self::parameter_rows(xtasks, command, focused, value_width);
                lines.extend(
                    ParameterList::new(
                        &rows,
                        SELECTABLE_FIELD_PREFIX_WIDTH,
                        label_width,
                        content_width,
                        parameter_list_styles(theme),
                    )
                    .render(),
                );
            }

            lines.push(Line::from(""));
            if let Some(about) = command
                .about
                .as_deref()
                .map(str::trim)
                .filter(|about| !about.is_empty())
            {
                lines.push(Line::styled(
                    fit_line_prefix(about, content_width),
                    theme.text(),
                ));
            }
            let manual = xtasks
                .run_request()
                .map(|request| request.command_line())
                .unwrap_or_else(|error| error.to_string());
            lines.push(Line::styled(
                fit_line_prefix(&manual, content_width),
                theme.accent(),
            ));
        }

        lines
    }

    fn title(&self) -> String {
        if self.app.xtasks.detail_open
            && let Some(command) = self.app.xtasks.selected_command()
        {
            return format!("Xtasks > {}", command.name);
        }
        "Xtasks".to_owned()
    }

    fn render_command_picker(&self, frame: &mut Frame<'_>, theme: &Theme, area: Rect) {
        let content_width = area.width as usize;
        let paragraph = Paragraph::new(Self::command_lines(&self.app.xtasks, theme, content_width))
            .style(theme.text())
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_detail(&self, frame: &mut Frame<'_>, theme: &Theme, area: Rect) {
        let areas = Self::detail_areas(area);
        let params_focused = self.app.xtasks.detail_focus == XtaskDetailFocus::Parameters;
        let output_focused = self.app.xtasks.detail_focus == XtaskDetailFocus::Output;

        self.render_parameters_panel(frame, theme, areas.parameters, params_focused);
        frame.render_widget(Clear, areas.separator);
        self.render_output_panel(frame, theme, areas.output, output_focused);
    }

    fn detail_areas(area: Rect) -> XtaskDetailAreas {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Length(1),
                Constraint::Percentage(60),
            ])
            .split(area);

        XtaskDetailAreas {
            parameters: chunks[0],
            separator: chunks[1],
            output: chunks[2],
        }
    }

    fn render_parameters_panel(
        &self,
        frame: &mut Frame<'_>,
        theme: &Theme,
        area: Rect,
        focused: bool,
    ) {
        let content_width = area.width.saturating_sub(2).max(1) as usize;
        let block = theme.panel_block("Parameters", None, focused);
        let inner = block.inner(area);
        let paragraph = scrollable_paragraph(
            Self::parameter_lines(&self.app.xtasks, theme, content_width, focused),
            theme,
            &self.app.xtasks.parameters_viewport,
        );
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(paragraph, inner);
    }

    fn render_output_panel(&self, frame: &mut Frame<'_>, theme: &Theme, area: Rect, focused: bool) {
        OutputPanel::new(
            &self.app.xtasks.output,
            self.app.xtasks.output_text(),
            self.output_label(),
            focused,
        )
        .render(frame, theme, area);
    }

    fn parameter_rows(
        xtasks: &XtaskState,
        command: &XtaskCommandSpec,
        focused: bool,
        value_width: usize,
    ) -> Vec<ParameterListRow> {
        command
            .args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                let selected = index == xtasks.selected_arg;
                let active = focused && selected;
                ParameterListRow {
                    selected,
                    active,
                    label: arg.flag(),
                    value: Self::arg_value_text(
                        xtasks,
                        &command.name,
                        &arg.name,
                        selected,
                        value_width,
                    ),
                    hint: arg.help.clone(),
                    details: Some(arg.value.parameter_details()),
                    ..Default::default()
                }
            })
            .collect()
    }

    fn param_label_width(command: &XtaskCommandSpec, content_width: usize) -> usize {
        let max_available = content_width
            .saturating_sub(SELECTABLE_FIELD_PREFIX_WIDTH + FIELD_HINT_MIN_COLUMN_WIDTH)
            .max(1);
        let cap = PARAM_LABEL_MAX_WIDTH.min(max_available).max(1);
        let longest = command
            .args
            .iter()
            .map(|arg| arg.flag().chars().count())
            .max()
            .unwrap_or(PARAM_LABEL_MIN_WIDTH);
        longest.max(PARAM_LABEL_MIN_WIDTH).min(cap)
    }

    fn output_label(&self) -> String {
        if self.app.xtasks.running {
            format!("Output: {}", self.app.running_test_spinner())
        } else if let Some(output) = &self.app.xtasks.last_run {
            if output.success {
                "Output: ✓".to_owned()
            } else {
                "Output: ✗".to_owned()
            }
        } else {
            "Output".to_owned()
        }
    }

    fn arg_value_text(
        xtasks: &XtaskState,
        command_name: &str,
        arg_name: &str,
        selected: bool,
        value_width: usize,
    ) -> String {
        if selected
            && let Some(editing) = &xtasks.editing
            && editing.command == command_name
            && editing.arg == arg_name
        {
            return format!(
                "[{}]",
                editing.input.view(value_width.saturating_sub(2), true)
            );
        }
        xtasks.arg_value_display(command_name, arg_name)
    }

    fn actions(xtasks: &XtaskState) -> &'static str {
        if xtasks.editing.is_some() {
            "[esc]cancel"
        } else if xtasks.detail_open {
            match xtasks.detail_focus {
                XtaskDetailFocus::Parameters => "[esc]back [tab]output [r]run",
                XtaskDetailFocus::Output => "[esc]back [tab]params [/]search [n/N]match [r]run",
            }
        } else {
            "[u]refresh [esc]close"
        }
    }
}
