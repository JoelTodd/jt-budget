use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Frame;
use ratatui::widgets::{Row, Table};

use super::super::layout::PanelChrome;
use super::super::theme::{Tone, UiTheme};
use super::super::widgets::{EditorFocusState, section_block};

pub(super) struct SectionTableSpec<'a, const N: usize> {
    pub widths: [Constraint; N],
    pub header: Row<'static>,
    pub title: Option<&'a str>,
    pub subtotal: String,
    pub focus: EditorFocusState,
    pub tone: Tone,
    pub chrome: PanelChrome,
}

pub(super) fn render_section_table<Rows, const N: usize>(
    frame: &mut Frame<'_>,
    area: Rect,
    rows: Rows,
    spec: SectionTableSpec<'_, N>,
    theme: &UiTheme,
) where
    Rows: IntoIterator<Item = Row<'static>>,
{
    let table = Table::new(rows, spec.widths)
        .header(spec.header.style(theme.emphasized_tone_style(spec.tone)))
        .style(theme.toned_panel_style(spec.tone))
        .block(section_block(
            spec.title,
            spec.subtotal,
            spec.focus,
            spec.tone,
            spec.chrome,
            theme,
        ));
    frame.render_widget(table, area);
}
