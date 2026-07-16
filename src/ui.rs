use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::{
    app::{App, FocusPane, FrameViewportMetrics, ViewportId, ViewportMetrics, ViewportSpec},
    command::OverlayMode,
    config,
    theme::Theme,
};

mod components;
mod geometry;
mod primitives;
mod view_helpers;

use components::discovery::DiscoveryModal;
use components::disk_cleanup::DiskCleanupModal;
use components::info::InfoPanel;
use components::output::OutputPanel;
#[cfg(test)]
use components::output::{output_actions, output_lines};
use components::output_search::OutputSearchModal;
use components::settings::SettingsModal;
use components::status::StatusBar;
use components::test_details::TestDetailsModal;
use components::test_events::TestEventsModal;
use components::tests::TestsPanel;
use components::xtasks::XtasksModal;
use geometry::{centered_rect, modal_inner_area, panel_body_page_size, panel_body_width};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppLayout {
    pub tree: Rect,
    pub details: Rect,
    pub output: Rect,
    pub status: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiGeometry {
    app_layout: AppLayout,
    viewports: FrameViewportMetrics,
}

impl UiGeometry {
    pub fn new(area: Rect, app: &App) -> Self {
        let app_layout = layout(area, app.settings.tree_width_percent);
        let xtask_inner = modal_inner_area(88, 82, area);
        let test_events_inner = modal_inner_area(88, 82, area);
        let test_details_inner = modal_inner_area(86, 88, area);
        let main_output_area = if app.discovery.error.is_some() {
            centered_rect(62, 58, area)
        } else {
            app_layout.output
        };
        let viewports = FrameViewportMetrics::new(vec![
            ViewportSpec::new(
                ViewportId::Tree,
                ViewportMetrics::new(panel_body_page_size(app_layout.tree)),
            ),
            ViewportSpec::new(
                ViewportId::MainOutput,
                output_viewport_metrics(main_output_area),
            ),
            ViewportSpec::new(
                ViewportId::XtaskParameters,
                ViewportMetrics::new(panel_body_page_size(XtasksModal::detail_parameters_area(
                    xtask_inner,
                ))),
            ),
            ViewportSpec::new(
                ViewportId::XtaskOutput,
                output_viewport_metrics(XtasksModal::detail_output_area(xtask_inner)),
            ),
            ViewportSpec::new(
                ViewportId::TestEventsOutput,
                output_viewport_metrics(TestEventsModal::output_area(test_events_inner)),
            ),
            ViewportSpec::new(
                ViewportId::TestStackSampleOutput,
                output_viewport_metrics(test_details_inner),
            ),
            ViewportSpec::new(
                ViewportId::TestDetails,
                ViewportMetrics::new(panel_body_page_size(centered_rect(86, 88, area))),
            ),
        ]);
        Self {
            app_layout,
            viewports,
        }
    }

    pub fn viewport_metrics(&self) -> FrameViewportMetrics {
        self.viewports.clone()
    }
}

pub fn layout(area: Rect, tree_width_percent: u16) -> AppLayout {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(area);

    let tree_width_percent = config::clamp_tree_width(tree_width_percent);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(tree_width_percent),
            Constraint::Percentage(100 - tree_width_percent),
        ])
        .split(outer[0]);

    let details_height = if panes[1].height < 14 {
        panes[1].height.saturating_sub(3).max(1)
    } else {
        12
    };

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(details_height), Constraint::Min(1)])
        .split(panes[1]);

    AppLayout {
        tree: panes[0],
        details: right[0],
        output: right[1],
        status: outer[1],
    }
}

#[cfg(test)]
pub fn viewport_metrics(area: Rect, app: &App) -> FrameViewportMetrics {
    UiGeometry::new(area, app).viewport_metrics()
}

fn output_viewport_metrics(area: Rect) -> ViewportMetrics {
    ViewportMetrics::new(panel_body_page_size(area)).with_content_width(panel_body_width(area))
}

#[cfg(test)]
pub fn draw(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let geometry = UiGeometry::new(frame.area(), app);
    draw_prepared(frame, app, theme, &geometry);
}

pub fn draw_prepared(frame: &mut Frame<'_>, app: &App, theme: &Theme, geometry: &UiGeometry) {
    let app_layout = geometry.app_layout;
    TestsPanel::new(app).render(frame, theme, app_layout.tree);
    InfoPanel::new(app).render(frame, theme, app_layout.details);
    draw_output(frame, app, theme, app_layout.output);
    StatusBar::new(app).render(frame, theme, app_layout.status);

    match app.command_context().overlay {
        Some(OverlayMode::Discovery | OverlayMode::DiscoveryError) => {
            DiscoveryModal::new(app).render(frame, theme);
        }
        Some(OverlayMode::OutputSearch) => {
            OutputSearchModal::new(app.active_output_search()).render(frame, theme);
        }
        Some(OverlayMode::DiskCleanup) => DiskCleanupModal::new(app).render(frame, theme),
        Some(OverlayMode::Xtasks) => XtasksModal::new(app).render(frame, theme),
        Some(OverlayMode::TestEvents) => TestEventsModal::new(app).render(frame, theme),
        Some(OverlayMode::TestDetails) => TestDetailsModal::new(app).render(frame, theme),
        Some(OverlayMode::Settings) => SettingsModal::new(app).render(frame, theme),
        None => {}
    }
    if app.xtasks.modal_open && app.xtasks.output.search.modal_open {
        OutputSearchModal::new(app.active_output_search()).render(frame, theme);
    }
    if app.test_events.modal_open && app.test_events.output.search.modal_open {
        OutputSearchModal::new(app.active_output_search()).render(frame, theme);
    }
    if app.show_test_details && app.test_stack_sample.output.search.modal_open {
        OutputSearchModal::new(app.active_output_search()).render(frame, theme);
    }
}

fn draw_output(frame: &mut Frame<'_>, app: &App, theme: &Theme, area: Rect) {
    OutputPanel::new(
        &app.main_output,
        app.output_source_text(),
        "Output",
        view_helpers::pane_focused(app, FocusPane::Output),
    )
    .render(frame, theme, area);
}

#[cfg(test)]
mod tests;
