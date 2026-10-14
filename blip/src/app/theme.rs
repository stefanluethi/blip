//! The light and dark themes: the colors of ayu light and ayu mirage
//! (<https://github.com/ayu-theme/ayu-colors>, version 8).

use egui::{Color32, Stroke, Theme, Visuals};

/// The colors of an ayu variant used by the user interface.
struct Palette {
    /// Side bars and panels.
    ui_bg: Color32,
    /// Weak text.
    ui_fg: Color32,
    /// Separators.
    ui_line: Color32,
    /// Popups and menus.
    panel_bg: Color32,
    panel_shadow: Color32,
    /// Text fields and plots.
    editor_bg: Color32,
    /// Normal text.
    editor_fg: Color32,
    /// Selected text and widgets, opaque on `editor_bg`.
    selection_bg: Color32,
    /// Readable on `selection_bg`, also marks the time cursor and drop targets.
    selection_fg: Color32,
    accent: Color32,
    warn: Color32,
    error: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// `color` with opacity `alpha` painted on `background`.
fn over(color: Color32, alpha: f32, background: Color32) -> Color32 {
    background.lerp_to_gamma(color, alpha)
}

fn ayu_light() -> Palette {
    let (ui_bg, editor_bg) = (hex(0xF8F9FA), hex(0xFCFCFC));
    let selection = hex(0x035BD6);
    Palette {
        ui_bg,
        ui_fg: hex(0x8A9199),
        ui_line: over(hex(0x6B7D8F), 0.12, ui_bg),
        panel_bg: hex(0xF3F4F5),
        panel_shadow: Color32::from_black_alpha(38),
        editor_bg,
        editor_fg: hex(0x5C6166),
        selection_bg: over(selection, 0.15, editor_bg),
        // the accent is too light to read on a light background
        selection_fg: selection,
        accent: hex(0xFFAA33),
        warn: hex(0xFA8D3E),
        error: hex(0xE65050),
    }
}

fn ayu_mirage() -> Palette {
    let (ui_bg, editor_bg) = (hex(0x1F2430), hex(0x242936));
    let accent = hex(0xFFCC66);
    Palette {
        ui_bg,
        ui_fg: hex(0x707A8C),
        ui_line: hex(0x171B24),
        panel_bg: hex(0x1C212B),
        panel_shadow: Color32::from_rgba_unmultiplied(0x12, 0x15, 0x1C, 179),
        editor_bg,
        editor_fg: hex(0xCCCAC2),
        selection_bg: over(hex(0x409FFF), 0.25, editor_bg),
        selection_fg: accent,
        accent,
        warn: hex(0xFFAD66),
        error: hex(0xFF6666),
    }
}

/// Uses ayu light for the light theme and ayu mirage for the dark theme.
pub fn install(ctx: &egui::Context) {
    ctx.set_visuals_of(Theme::Light, visuals(&ayu_light(), Visuals::light()));
    ctx.set_visuals_of(Theme::Dark, visuals(&ayu_mirage(), Visuals::dark()));
}

/// `base` with the colors of `palette`.
fn visuals(palette: &Palette, base: Visuals) -> Visuals {
    let p = palette;
    // the shade of widgets: the weak text color painted on the panels
    let shade = |alpha| over(p.ui_fg, alpha, p.ui_bg);
    let contrast = if base.dark_mode {
        Color32::WHITE
    } else {
        Color32::BLACK
    };
    let strong_fg = p.editor_fg.lerp_to_gamma(contrast, 0.5);

    let mut widgets = base.widgets;
    let states = [
        (&mut widgets.noninteractive, p.ui_bg, p.ui_line, p.editor_fg),
        (
            &mut widgets.inactive,
            shade(0.2),
            Color32::TRANSPARENT,
            p.editor_fg,
        ),
        (&mut widgets.hovered, shade(0.3), p.ui_fg, strong_fg),
        (&mut widgets.active, shade(0.4), strong_fg, strong_fg),
        (&mut widgets.open, shade(0.3), p.ui_line, strong_fg),
    ];
    for (state, fill, stroke, text) in states {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke.color = stroke;
        state.fg_stroke.color = text;
    }

    let mut text_cursor = base.text_cursor;
    text_cursor.stroke.color = p.accent;
    let mut window_shadow = base.window_shadow;
    window_shadow.color = p.panel_shadow;
    let mut popup_shadow = base.popup_shadow;
    popup_shadow.color = p.panel_shadow;

    Visuals {
        weak_text_color: Some(p.ui_fg),
        widgets,
        selection: egui::style::Selection {
            bg_fill: p.selection_bg,
            stroke: Stroke::new(base.selection.stroke.width, p.selection_fg),
        },
        hyperlink_color: p.accent,
        faint_bg_color: shade(0.08),
        extreme_bg_color: p.editor_bg,
        code_bg_color: shade(0.12),
        warn_fg_color: p.warn,
        error_fg_color: p.error,
        window_shadow,
        window_fill: p.panel_bg,
        window_stroke: Stroke::new(base.window_stroke.width, p.ui_line),
        panel_fill: p.ui_bg,
        popup_shadow,
        text_cursor,
        ..base
    }
}
