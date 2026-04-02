use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Border treatment used to keep wide layouts less visually dense.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelChrome {
    Boxed,
    TopRule,
}

/// Returns a centred popup rectangle expressed as percentages of the frame.
pub(super) fn centred_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Guided-creation layout buckets tuned for the supported terminal sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GuidedLayoutProfile {
    Compact,
    Standard,
    Wide,
}

impl GuidedLayoutProfile {
    pub(super) fn for_area(area: Rect) -> Self {
        if area.width >= 160 && area.height >= 32 {
            Self::Wide
        } else if area.width >= 100 && area.height >= 32 {
            Self::Standard
        } else {
            Self::Compact
        }
    }
}

/// Monthly-editor layout buckets tuned for the supported terminal sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EditorLayoutProfile {
    Compact,
    Standard,
    Wide,
}

impl EditorLayoutProfile {
    pub(super) fn for_area(area: Rect) -> Self {
        if area.width >= 160 && area.height >= 32 {
            Self::Wide
        } else if area.width >= 100 && area.height >= 32 {
            Self::Standard
        } else {
            Self::Compact
        }
    }
}

/// Estimates the table height needed for a section plus its header and borders.
pub(super) fn section_height(row_count: usize, compact_title: bool) -> u16 {
    let base = row_count as u16 + if compact_title { 3 } else { 4 };
    base.max(6)
}
