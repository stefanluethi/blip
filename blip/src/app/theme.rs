//! The look of blip: the light and dark "Control Room" themes, with the views as cards
//! on a darker canvas, the IBM colour-blind-safe palette for the traces, and Fira Sans
//! and Fira Code for the text, as in the documentation.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Shadow, Stroke};
use egui::{FontId, TextStyle, Theme, Visuals};
use std::collections::BTreeMap;

/// The corner radius of the cards of the views.
pub const CARD_RADIUS: u8 = 4;
/// The corner radius of buttons, fields and other controls.
pub const CONTROL_RADIUS: u8 = 3;
/// The corner radius of menus, popups and tooltips.
const POPUP_RADIUS: u8 = 4;
/// The height of buttons, fields, checkboxes and the rows of trees, so that they line up.
pub const CONTROL_HEIGHT: f32 = 24.0;
/// The margin of text fields, which makes them as high as the other controls with the
/// body text: the text, these margins and the stroke.
pub const FIELD_MARGIN: egui::Margin = egui::Margin::symmetric(6, 3);
/// The gap between the cards of the views and around them.
pub const GAP: f32 = 8.0;

/// The colours of a theme.
pub struct Tokens {
    /// Behind the cards of the views.
    pub canvas: Color32,
    /// The side, top and bottom panels.
    panel: Color32,
    /// The cards of the views, also behind the plots.
    pub card: Color32,
    /// Buttons, checkboxes and other controls.
    pub control: Color32,
    /// A control under the pointer.
    hover: Color32,
    /// Menus, popups and tooltips.
    popup: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    text: Color32,
    /// Secondary text, like labels and hints.
    text_weak: Color32,
    /// Text that stays in the background, like the times of log entries.
    pub text_faint: Color32,
    /// The colour of selection, focus and the live button.
    pub accent: Color32,
    /// Text on the accent.
    pub on_accent: Color32,
    /// The time cursor, apart from the trace colours, most of which are saturated.
    cursor: Color32,
    /// The accent as text and lines on the canvas, panels and cards.
    accent_text: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub error: Color32,
    pub debug: Color32,
    pub trace: Color32,
    /// The colours of the traces, in order: the IBM colour-blind-safe palette ordered
    /// so that neighbours differ most, and a grey as the sixth.
    traces: [Color32; 6],
    /// How much of a colour tints the card, e.g. for the bands of states.
    pub tint: f32,
}

pub const LIGHT: Tokens = Tokens {
    canvas: Color32::from_rgb(0xf4, 0xf5, 0xf5),
    panel: Color32::from_rgb(0xff, 0xff, 0xff),
    card: Color32::from_rgb(0xff, 0xff, 0xff),
    control: Color32::from_rgb(0xf4, 0xf5, 0xf5),
    hover: Color32::from_rgb(0xec, 0xee, 0xf0),
    popup: Color32::from_rgb(0xff, 0xff, 0xff),
    border: Color32::from_rgb(0xdc, 0xde, 0xe1),
    border_strong: Color32::from_rgb(0xc7, 0xca, 0xcf),
    text: Color32::from_rgb(0x24, 0x29, 0x2e),
    text_weak: Color32::from_rgb(0x5b, 0x61, 0x68),
    text_faint: Color32::from_rgb(0x87, 0x8d, 0x95),
    accent: Color32::from_rgb(0x38, 0x71, 0xdc),
    cursor: Color32::from_rgb(0x3a, 0x3f, 0x46),
    on_accent: Color32::WHITE,
    accent_text: Color32::from_rgb(0x1f, 0x62, 0xe0),
    ok: Color32::from_rgb(0x1f, 0x9d, 0x55),
    warn: Color32::from_rgb(0xc2, 0x7c, 0x0e),
    error: Color32::from_rgb(0xd9, 0x3a, 0x3a),
    debug: Color32::from_rgb(0x7b, 0x61, 0xd9),
    trace: Color32::from_rgb(0x8e, 0x91, 0x99),
    traces: [
        Color32::from_rgb(0x64, 0x8f, 0xff),
        Color32::from_rgb(0xfe, 0x61, 0x00),
        Color32::from_rgb(0xdc, 0x26, 0x7f),
        Color32::from_rgb(0xff, 0xb0, 0x00),
        Color32::from_rgb(0x78, 0x5e, 0xf0),
        Color32::from_rgb(0x6b, 0x72, 0x80),
    ],
    tint: 0.3,
};

pub const DARK: Tokens = Tokens {
    canvas: Color32::from_rgb(0x11, 0x12, 0x17),
    panel: Color32::from_rgb(0x18, 0x1b, 0x1f),
    card: Color32::from_rgb(0x18, 0x1b, 0x1f),
    control: Color32::from_rgb(0x22, 0x25, 0x2b),
    hover: Color32::from_rgb(0x2a, 0x2d, 0x34),
    popup: Color32::from_rgb(0x22, 0x25, 0x2b),
    border: Color32::from_rgb(0x2a, 0x2d, 0x33),
    border_strong: Color32::from_rgb(0x3a, 0x3e, 0x46),
    text: Color32::from_rgb(0xcc, 0xcc, 0xdc),
    text_weak: Color32::from_rgb(0x9a, 0x9a, 0xab),
    text_faint: Color32::from_rgb(0x6e, 0x6e, 0x80),
    accent: Color32::from_rgb(0x3d, 0x71, 0xd9),
    cursor: Color32::from_rgb(0xec, 0xec, 0xf4),
    on_accent: Color32::WHITE,
    accent_text: Color32::from_rgb(0x6e, 0x9f, 0xff),
    ok: Color32::from_rgb(0x3f, 0xbf, 0x72),
    warn: Color32::from_rgb(0xe3, 0xa9, 0x3a),
    error: Color32::from_rgb(0xf2, 0x61, 0x5b),
    debug: Color32::from_rgb(0xa6, 0x8b, 0xfa),
    trace: Color32::from_rgb(0x80, 0x83, 0x8b),
    traces: [
        Color32::from_rgb(0x64, 0x8f, 0xff),
        Color32::from_rgb(0xfe, 0x61, 0x00),
        Color32::from_rgb(0xdc, 0x26, 0x7f),
        Color32::from_rgb(0xff, 0xb0, 0x00),
        Color32::from_rgb(0x78, 0x5e, 0xf0),
        Color32::from_rgb(0x9a, 0xa0, 0xa6),
    ],
    tint: 0.42,
};

/// The colours of the theme of `visuals`.
pub fn tokens(visuals: &Visuals) -> &'static Tokens {
    if visuals.dark_mode { &DARK } else { &LIGHT }
}

/// Installs the fonts, their sizes and the light and the dark theme.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_visuals_of(Theme::Light, visuals(&LIGHT, Visuals::light()));
    ctx.set_visuals_of(Theme::Dark, visuals(&DARK, Visuals::dark()));
    let text_styles: BTreeMap<TextStyle, FontId> = [
        (TextStyle::Heading, FontId::proportional(20.0)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Small, FontId::proportional(11.0)),
    ]
    .into();
    ctx.all_styles_mut(move |style| {
        style.text_styles = text_styles.clone();
        let spacing = &mut style.spacing;
        spacing.interact_size.y = CONTROL_HEIGHT;
        spacing.button_padding = egui::vec2(8.0, 2.0);
        spacing.item_spacing = egui::vec2(8.0, 3.0);
        spacing.indent = 16.0;
        spacing.menu_margin = egui::Margin::same(6);
    });
}

/// Fira Sans for text and Fira Code for values, both subset (see `assets/fonts`), then
/// the Phosphor icons and egui's default fonts for the glyphs they lack.
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        let data = FontData::from_static(bytes);
        fonts.font_data.insert(name.to_owned(), data.into());
    };
    add(
        "Fira Sans",
        include_bytes!("../../assets/fonts/FiraSans-Regular.ttf"),
    );
    add(
        "Fira Code",
        include_bytes!("../../assets/fonts/FiraCode-Regular.ttf"),
    );
    add("phosphor", egui_phosphor::Variant::Regular.font_bytes());
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "Fira Sans".to_owned());
    proportional.insert(1, "phosphor".to_owned());
    let monospace = fonts.families.entry(FontFamily::Monospace).or_default();
    monospace.insert(0, "Fira Code".to_owned());
    monospace.push("phosphor".to_owned());
    fonts
}

/// `base` in the colours of `t`.
fn visuals(t: &Tokens, base: Visuals) -> Visuals {
    let dark = base.dark_mode;
    let mut v = base;
    v.panel_fill = t.panel;
    // egui_plot paints its background with it, so the plots blend into their cards
    v.extreme_bg_color = t.card;
    v.text_edit_bg_color = Some(t.canvas);
    v.faint_bg_color = t.control;
    v.code_bg_color = t.control;
    v.window_fill = t.popup;
    v.window_stroke = Stroke::new(1.0, t.border);
    v.window_corner_radius = CornerRadius::same(POPUP_RADIUS);
    v.menu_corner_radius = CornerRadius::same(POPUP_RADIUS);
    let shadow_alpha = if dark { 120 } else { 28 };
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    // the accent mixed into the panel, so selected text and rows stay readable
    v.selection.bg_fill = t
        .panel
        .lerp_to_gamma(t.accent, if dark { 0.35 } else { 0.18 });
    v.selection.stroke = Stroke::new(1.0, t.accent_text);
    v.hyperlink_color = t.accent_text;
    v.warn_fg_color = t.warn;
    v.error_fg_color = t.error;
    v.weak_text_color = Some(t.text_weak);
    // the indentation alone shows the levels of trees
    v.indent_has_left_vline = false;

    let radius = CornerRadius::same(CONTROL_RADIUS);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = t.panel;
    w.noninteractive.weak_bg_fill = t.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
    w.noninteractive.corner_radius = radius;
    w.inactive.bg_fill = t.control;
    w.inactive.weak_bg_fill = t.control;
    w.inactive.bg_stroke = Stroke::new(1.0, t.border);
    w.inactive.fg_stroke = Stroke::new(1.0, t.text);
    w.inactive.corner_radius = radius;
    w.hovered.bg_fill = t.hover;
    w.hovered.weak_bg_fill = t.hover;
    w.hovered.bg_stroke = Stroke::new(1.0, t.border_strong);
    w.hovered.fg_stroke = Stroke::new(1.5, t.text);
    w.hovered.corner_radius = radius;
    w.active.bg_fill = t.accent;
    w.active.weak_bg_fill = t.hover.lerp_to_gamma(t.text, 0.06);
    w.active.bg_stroke = Stroke::new(1.0, t.accent);
    w.active.fg_stroke = Stroke::new(2.0, t.text);
    w.active.corner_radius = radius;
    w.open = w.hovered;
    v
}

/// The colour of the `index`th trace of a view.
pub fn trace_color(visuals: &Visuals, index: usize) -> Color32 {
    let traces = &tokens(visuals).traces;
    traces.get(index).copied().unwrap_or_else(|| {
        // past the palette, hues spread by the golden ratio stay apart
        let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0;
        let value = if visuals.dark_mode { 0.9 } else { 0.7 };
        egui::ecolor::Hsva::new(index as f32 * golden_ratio, 0.6, value, 1.0).into()
    })
}

/// The `index`th colour of the palette, wrapping around its distinct colours (the grey
/// is left out), e.g. for the values of states.
pub fn palette_color(visuals: &Visuals, index: usize) -> Color32 {
    let traces = &tokens(visuals).traces;
    traces[index % (traces.len() - 1)]
}

/// The grey of the palette, for values that should not stand out.
pub fn palette_grey(visuals: &Visuals) -> Color32 {
    tokens(visuals).traces[5]
}

/// The color of the time cursor: light on the dark theme, dark on the light one.
pub fn cursor_color(visuals: &Visuals) -> Color32 {
    tokens(visuals).cursor
}

/// The title of a panel or of a section of properties: small capitals in the weak text
/// color.
pub fn title(visuals: &Visuals, text: &str) -> egui::RichText {
    egui::RichText::new(text.to_uppercase())
        .size(11.0)
        .strong()
        .color(tokens(visuals).text_weak)
}

/// Chooses one of `options` by their names, side by side in a sunken track with the
/// chosen one raised. Like Rerun's selectable toggle, the buttons are made as high as
/// the track leaves room for, so that their text is centered. Returns whether the choice
/// changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let t = tokens(ui.visuals());
    let margin = 2;
    let mut changed = false;
    egui::Frame::NONE
        .fill(t.canvas)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CONTROL_RADIUS)
        .inner_margin(margin)
        .show(ui, |ui| {
            // the track keeps the height of the other controls
            let height = ui.spacing().interact_size.y - 2.0 * (f32::from(margin) + 1.0);
            ui.spacing_mut().interact_size.y = height;
            ui.spacing_mut().button_padding = egui::vec2(4.0, 0.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            let count = options.len().max(1) as f32;
            let width = ((ui.available_width() - 2.0 * (count - 1.0)) / count).max(0.0);
            ui.horizontal(|ui| {
                for &(option, name) in options {
                    let chosen = *value == option;
                    let color = if chosen { t.text } else { t.text_weak };
                    let text = egui::RichText::new(name).size(12.0).color(color);
                    let mut button =
                        egui::Button::selectable(chosen, text).corner_radius(CONTROL_RADIUS - 1);
                    if chosen {
                        button = button
                            .fill(t.control)
                            .stroke(Stroke::new(1.0, t.border_strong));
                    }
                    let response = ui
                        .allocate_ui_with_layout(
                            egui::vec2(width, height),
                            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| ui.add(button),
                        )
                        .inner;
                    if response.clicked() && !chosen {
                        *value = option;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// A row with the title of a section of a panel, and the buttons added by `add` on the
/// right, as high as the controls.
pub fn section_header(ui: &mut egui::Ui, text: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(CONTROL_HEIGHT);
        ui.label(title(ui.visuals(), text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
}

/// A button with only `icon`, framed only while hovered, like the add buttons of the
/// panels.
pub fn icon_button(icon: &str) -> egui::Button<'_> {
    egui::Button::new(icon).frame_when_inactive(false)
}

/// The room above and below the text of a table row.
const TABLE_ROW_PADDING: f32 = 10.0;

/// A table in `ui` with the look of all tables: no stripes, and the cells centered
/// vertically. [`TableLines`] draws the lines between its rows.
pub fn table(ui: &mut egui::Ui) -> egui_extras::TableBuilder<'_> {
    egui_extras::TableBuilder::new(ui)
        .striped(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
}

/// The height of a table row with `lines` lines of `style`.
pub fn table_row_height(ui: &egui::Ui, style: &TextStyle, lines: usize) -> f32 {
    let extra_lines = lines.saturating_sub(1) as f32 * ui.text_style_height(style);
    style.resolve(ui.style()).size + extra_lines + TABLE_ROW_PADDING
}

/// The lines between the rows of a table, which run across all its columns, and below
/// its header if it has one.
#[derive(Clone, Copy)]
pub struct TableLines {
    span: egui::Rangef,
    stroke: Stroke,
    header: bool,
    clip: egui::Rect,
}

impl TableLines {
    /// The lines of a table about to be built in `ui`. The borders of resizable columns
    /// of the table show only while hovered or dragged.
    pub fn new(ui: &mut egui::Ui, header: bool) -> Self {
        let lines = Self {
            span: ui.max_rect().x_range(),
            stroke: ui.visuals().widgets.noninteractive.bg_stroke,
            header,
            clip: egui::Rect::EVERYTHING,
        };
        ui.visuals_mut().widgets.noninteractive.bg_stroke = Stroke::NONE;
        lines
    }

    /// The lines clipped to the scrolled `body` of the table, also of its clipped cells,
    /// and to the gap below the header.
    pub fn in_body(mut self, body: &egui::Ui) -> Self {
        self.clip = body.clip_rect();
        if self.header {
            self.clip.min.y -= body.spacing().item_spacing.y;
        }
        self
    }

    /// Draws the line above the `row`th row of the body with `cell`, a cell of the row.
    /// The first row of a table without a header has none.
    pub fn above(&self, cell: &egui::Ui, row: usize) {
        if row == 0 && !self.header {
            return;
        }
        let y = cell.max_rect().top() - cell.spacing().item_spacing.y / 2.0;
        let mut painter = cell.painter().clone();
        painter.set_clip_rect(self.clip);
        painter.hline(self.span, y, self.stroke);
    }
}

/// The color of the grid lines of plots, which egui_plot fades for denser lines.
pub fn grid_color(visuals: &Visuals) -> Color32 {
    tokens(visuals).border_strong
}

/// `color` tinting a card, readable under text.
pub fn tint(visuals: &Visuals, color: Color32) -> Color32 {
    let t = tokens(visuals);
    t.card.lerp_to_gamma(color, t.tint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_traces_take_the_palette_and_later_ones_still_differ() {
        let visuals = Visuals::dark();
        assert_eq!(trace_color(&visuals, 0), DARK.traces[0]);
        let colors: std::collections::BTreeSet<[u8; 4]> = (0..12)
            .map(|i| trace_color(&visuals, i).to_array())
            .collect();
        assert_eq!(colors.len(), 12);
    }

    #[test]
    fn the_palette_wraps_without_the_grey() {
        let visuals = Visuals::light();
        assert_eq!(palette_color(&visuals, 5), LIGHT.traces[0]);
        assert!((0..5).all(|i| palette_color(&visuals, i) != palette_grey(&visuals)));
    }
}
