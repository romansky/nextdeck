use ratatui::{
    Frame,
    text::{Line, Span},
};

use crate::{
    app::{App, FocusPane},
    command::{CommandGroup, CommandInfo, command_infos},
    theme::Theme,
};

use super::super::{
    geometry::centered_rect,
    primitives::{ModalChrome, draw_modal_shell, scrollable_paragraph},
};

pub(in crate::ui) struct HelpModal<'a> {
    app: &'a App,
}

impl<'a> HelpModal<'a> {
    pub(in crate::ui) fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme) {
        let area = centered_rect(72, 96, frame.area());
        let text = Self::text(theme, self.app.focus);
        let inner = draw_modal_shell(
            frame,
            theme,
            area,
            ModalChrome {
                title: "Help",
                actions: Some("[pgUp/pgDn]scroll [h/?/F1]close [esc]close [q]close"),
            },
        );
        frame.render_widget(
            scrollable_paragraph(text, theme, &self.app.help_viewport),
            inner,
        );
    }

    pub(in crate::ui) fn content_len(_: FocusPane) -> usize {
        1 + 2
            + 1
            + Self::command_count(CommandGroup::Navigation)
            + Self::command_count(CommandGroup::Global)
            + 2
            + 1
            + Self::command_count(CommandGroup::Runs)
            + 1
            + Self::command_count(CommandGroup::View)
            + 2
            + Self::command_count(CommandGroup::Output)
    }

    pub(in crate::ui) fn text(theme: &Theme, focus: FocusPane) -> Vec<Line<'static>> {
        let mut text = Vec::new();
        text.push(Line::from(vec![
            Span::styled("NextDeck", theme.title(true)),
            Span::raw(" "),
            Span::styled(env!("CARGO_PKG_VERSION"), theme.muted()),
        ]));
        Self::append_section(&mut text, "Global", true, theme);
        Self::append_group(&mut text, CommandGroup::Navigation, true, theme);
        Self::append_commands(&mut text, CommandGroup::Global, true, theme);

        let tests_active = focus == FocusPane::Tree;
        Self::append_section(&mut text, "Tests", tests_active, theme);
        Self::append_group(&mut text, CommandGroup::Runs, tests_active, theme);
        Self::append_group(&mut text, CommandGroup::View, tests_active, theme);

        let output_active = focus == FocusPane::Output;
        Self::append_section(&mut text, "Output", output_active, theme);
        Self::append_commands(&mut text, CommandGroup::Output, output_active, theme);

        text
    }

    fn command_count(group: CommandGroup) -> usize {
        command_infos()
            .iter()
            .filter(|info| info.group == group)
            .count()
    }

    fn append_section(
        text: &mut Vec<Line<'static>>,
        title: &'static str,
        active: bool,
        theme: &Theme,
    ) {
        if !text.is_empty() {
            text.push(Line::from(""));
        }
        text.push(Line::styled(
            title,
            if active {
                theme.title(true)
            } else {
                theme.muted()
            },
        ));
    }

    fn append_group(
        text: &mut Vec<Line<'static>>,
        group: CommandGroup,
        active: bool,
        theme: &Theme,
    ) {
        text.push(Line::styled(
            format!("  {}", group.title()),
            if active {
                theme.accent()
            } else {
                theme.muted()
            },
        ));
        Self::append_commands(text, group, active, theme);
    }

    fn append_commands(
        text: &mut Vec<Line<'static>>,
        group: CommandGroup,
        active: bool,
        theme: &Theme,
    ) {
        let mut infos = command_infos()
            .iter()
            .filter(|info| info.group == group)
            .collect::<Vec<_>>();
        infos.sort_by_key(|info| Self::sort_text(info));
        for info in infos {
            text.push(Self::line(info, active, theme));
        }
    }

    fn sort_text(info: &CommandInfo) -> String {
        format!("{} {}", info.keys.to_ascii_lowercase(), info.label)
    }

    fn line(info: &CommandInfo, active: bool, theme: &Theme) -> Line<'static> {
        let key_style = if active {
            theme.accent()
        } else {
            theme.muted()
        };
        let label_style = if active { theme.text() } else { theme.muted() };
        Line::from(vec![
            Span::raw("    "),
            Span::styled(format!("{:<15}", info.keys), key_style),
            Span::raw(" "),
            Span::styled(info.label, label_style),
        ])
    }
}
