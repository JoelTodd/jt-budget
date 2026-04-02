use std::sync::OnceLock;

use ratatui::prelude::{Color, Modifier, Span, Style};
use serde::Deserialize;

use crate::state::SectionId;

/// Semantic colour roles used by the TUI instead of hard-coding palette slots at
/// individual call sites.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Navigation,
    Guided,
    Summary,
    Status,
    Liability,
    Accounts,
    Timing,
    Earmarks,
    Pots,
    Danger,
    Success,
    Warning,
}

type RgbTriplet = (u8, u8, u8);

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ThemeConfig {
    pub(super) base24: Base24PaletteConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Base24PaletteConfig {
    pub(super) base00: String,
    pub(super) base01: String,
    pub(super) base02: String,
    pub(super) base03: String,
    pub(super) base04: String,
    pub(super) base05: String,
    pub(super) base06: String,
    pub(super) base07: String,
    pub(super) base08: String,
    pub(super) base09: String,
    #[serde(rename = "base0A")]
    pub(super) base0_a: String,
    #[serde(rename = "base0B")]
    pub(super) base0_b: String,
    #[serde(rename = "base0C")]
    pub(super) base0_c: String,
    #[serde(rename = "base0D")]
    pub(super) base0_d: String,
    #[serde(rename = "base0E")]
    pub(super) base0_e: String,
    #[serde(rename = "base0F")]
    pub(super) base0_f: String,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct UiTheme {
    pub(super) base00: Color,
    pub(super) base03: Color,
    pub(super) base04: Color,
    pub(super) base05: Color,
    pub(super) base06: Color,
    pub(super) base08: Color,
    pub(super) base09: Color,
    pub(super) base0a: Color,
    pub(super) base0b: Color,
    pub(super) base0c: Color,
    pub(super) base0d: Color,
    pub(super) base0e: Color,
}

/// Helper for constructing [`Color::Rgb`] values in tests and palette parsing.
pub(super) fn rgb(colour: RgbTriplet) -> Color {
    Color::Rgb(colour.0, colour.1, colour.2)
}

impl UiTheme {
    pub(super) fn project_default() -> &'static Self {
        static THEME: OnceLock<UiTheme> = OnceLock::new();
        THEME.get_or_init(|| {
            let config: ThemeConfig = toml::from_str(include_str!("../../theme.toml"))
                .expect("project theme.toml is valid");
            config
                .base24
                .validate()
                .expect("project theme.toml passes base24 validation");
            Self::from_palette(&config.base24)
        })
    }

    /// Builds a render theme from the validated Base24 palette config.
    pub(super) fn from_palette(palette: &Base24PaletteConfig) -> Self {
        Self {
            base00: rgb(parse_hex_triplet(&palette.base00)),
            base03: rgb(parse_hex_triplet(&palette.base03)),
            base04: rgb(parse_hex_triplet(&palette.base04)),
            base05: rgb(parse_hex_triplet(&palette.base05)),
            base06: rgb(parse_hex_triplet(&palette.base06)),
            base08: rgb(parse_hex_triplet(&palette.base08)),
            base09: rgb(parse_hex_triplet(&palette.base09)),
            base0a: rgb(parse_hex_triplet(&palette.base0_a)),
            base0b: rgb(parse_hex_triplet(&palette.base0_b)),
            base0c: rgb(parse_hex_triplet(&palette.base0_c)),
            base0d: rgb(parse_hex_triplet(&palette.base0_d)),
            base0e: rgb(parse_hex_triplet(&palette.base0_e)),
        }
    }

    fn tone_colour(&self, tone: Tone) -> Color {
        match tone {
            Tone::Navigation => self.base0d,
            Tone::Guided => self.base0a,
            Tone::Summary => self.base0c,
            Tone::Status => self.base0a,
            Tone::Liability => self.base0e,
            Tone::Accounts => self.base0d,
            Tone::Timing => self.base09,
            Tone::Earmarks => self.base0e,
            Tone::Pots => self.base0b,
            Tone::Danger => self.base08,
            Tone::Success => self.base0b,
            Tone::Warning => self.base09,
        }
    }

    fn panel_surface_colour(&self, _tone: Tone) -> Color {
        self.base00
    }

    pub(super) fn app_style(&self) -> Style {
        Style::default().bg(self.base00).fg(self.base05)
    }

    pub(super) fn toned_panel_style(&self, tone: Tone) -> Style {
        Style::default()
            .bg(self.panel_surface_colour(tone))
            .fg(self.base05)
    }

    pub(super) fn muted_style(&self) -> Style {
        Style::default().fg(self.base04)
    }

    pub(super) fn subtle_style(&self) -> Style {
        Style::default().fg(self.base03)
    }

    pub(super) fn bright_style(&self) -> Style {
        Style::default().fg(self.base06)
    }

    pub(super) fn tone_style(&self, tone: Tone) -> Style {
        Style::default().fg(self.tone_colour(tone))
    }

    pub(super) fn emphasized_tone_style(&self, tone: Tone) -> Style {
        self.tone_style(tone).add_modifier(Modifier::BOLD)
    }

    pub(super) fn panel_border_style(&self, tone: Tone) -> Style {
        self.tone_style(tone)
    }

    pub(super) fn selected_style(&self) -> Style {
        Style::default()
            .bg(self.base00)
            .fg(self.base06)
            .add_modifier(Modifier::BOLD)
    }

    pub(super) fn editing_style(&self) -> Style {
        Style::default()
            .bg(self.base00)
            .fg(self.tone_colour(Tone::Warning))
            .add_modifier(Modifier::BOLD)
    }
}

impl Base24PaletteConfig {
    pub(super) fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("base24.base00", self.base00.as_str()),
            ("base24.base01", self.base01.as_str()),
            ("base24.base02", self.base02.as_str()),
            ("base24.base03", self.base03.as_str()),
            ("base24.base04", self.base04.as_str()),
            ("base24.base05", self.base05.as_str()),
            ("base24.base06", self.base06.as_str()),
            ("base24.base07", self.base07.as_str()),
            ("base24.base08", self.base08.as_str()),
            ("base24.base09", self.base09.as_str()),
            ("base24.base0A", self.base0_a.as_str()),
            ("base24.base0B", self.base0_b.as_str()),
            ("base24.base0C", self.base0_c.as_str()),
            ("base24.base0D", self.base0_d.as_str()),
            ("base24.base0E", self.base0_e.as_str()),
            ("base24.base0F", self.base0_f.as_str()),
        ] {
            validate_hex_colour(field, value)?;
        }

        Ok(())
    }
}

fn validate_hex_colour(field: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix('#') else {
        return Err(format!("{field} must use #RRGGBB"));
    };
    if hex.len() != 6 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(format!("{field} must use #RRGGBB"));
    }

    Ok(())
}

pub(super) fn parse_hex_triplet(colour: &str) -> RgbTriplet {
    let hex = colour.strip_prefix('#').expect("base24 palette validated");
    (
        u8::from_str_radix(&hex[0..2], 16).expect("base24 palette validated"),
        u8::from_str_radix(&hex[2..4], 16).expect("base24 palette validated"),
        u8::from_str_radix(&hex[4..6], 16).expect("base24 palette validated"),
    )
}

/// Maps editor sections onto their semantic accent tones.
pub(super) fn section_tone(section: SectionId) -> Tone {
    match section {
        SectionId::Accounts => Tone::Accounts,
        SectionId::TimingAdjustments => Tone::Timing,
        SectionId::NextMonthEarmarks => Tone::Earmarks,
        SectionId::SavingsPots => Tone::Pots,
    }
}

/// Chooses the tone used for validation-related status.
pub(super) fn validation_tone(is_valid: bool) -> Tone {
    if is_valid {
        Tone::Success
    } else {
        Tone::Danger
    }
}

pub(super) fn metric_spans(
    theme: &UiTheme,
    label: &str,
    value: String,
    tone: Tone,
) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("{label} "), theme.muted_style()),
        Span::styled(value, theme.emphasized_tone_style(tone)),
    ]
}

pub(super) fn status_value_style(theme: &UiTheme, label: &str) -> Style {
    match label {
        "clean" | "synced" | "valid" | "within tolerance" => {
            theme.emphasized_tone_style(Tone::Success)
        }
        "dirty" | "pending" => theme.emphasized_tone_style(Tone::Warning),
        "autosaving" | "syncing" => theme.emphasized_tone_style(Tone::Status),
        "failed" | "invalid" | "outside tolerance" => theme.emphasized_tone_style(Tone::Danger),
        _ => theme.bright_style(),
    }
}

pub(super) fn operational_status_style(theme: &UiTheme, label: &str) -> Style {
    match label {
        "clean" | "synced" => theme.tone_style(Tone::Status),
        "dirty" | "pending" => theme.tone_style(Tone::Warning),
        "autosaving" | "syncing" => theme.emphasized_tone_style(Tone::Status),
        "failed" => theme.emphasized_tone_style(Tone::Danger),
        _ => theme.muted_style(),
    }
}

pub(super) fn month_state_style(theme: &UiTheme, is_valid: bool) -> Style {
    if is_valid {
        theme.tone_style(Tone::Success)
    } else {
        theme.tone_style(Tone::Warning)
    }
}

pub(super) fn key_hint_spans(theme: &UiTheme, hint: &str) -> Vec<Span<'static>> {
    if let Some((key, action)) = hint.split_once(' ') {
        vec![
            Span::styled(key.to_owned(), theme.emphasized_tone_style(Tone::Guided)),
            Span::raw(" "),
            Span::styled(action.to_owned(), theme.muted_style()),
        ]
    } else {
        vec![Span::styled(
            hint.to_owned(),
            theme.emphasized_tone_style(Tone::Guided),
        )]
    }
}
