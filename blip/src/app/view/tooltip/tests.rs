use super::*;
use crate::app::theme;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn trace(name: &str, path: &str, index: usize) -> TraceName {
    TraceName {
        name: name.into(),
        path: path.into(),
        index,
    }
}

#[test]
fn the_values_line_up_right_in_columns_as_wide_as_the_widest() {
    let tooltip = Tooltip::new(Header::Text("0:00:30.000".into()))
        .group(vec![
            Row::new(
                Label::Trace(trace("speed", "motor/speed", 0)),
                [Value::text("1200 rpm")],
            ),
            Row::new(Label::Weak("[3]".into()), [Value::text("7 rpm")]),
        ])
        .group(vec![
            Row::new(Label::Weak("Samples".into()), [Value::text("42")]),
            Row::new(Label::Weak("Outliers".into()), [Value::faint("0")]),
        ]);
    let mut harness = Harness::new_ui(|ui| {
        theme::install(ui.ctx());
        tooltip.ui(ui);
    });
    harness.run();
    let rect = |label: &str| harness.get_by_label(label).rect();
    let right = rect("1200 rpm").right();
    for value in ["7 rpm", "42", "0"] {
        assert!(
            (rect(value).right() - right).abs() < 0.5,
            "{value} ends at {}, not at {right}",
            rect(value).right()
        );
    }
    assert!(rect("0:00:30.000").bottom() <= rect("speed").top());
    assert!(rect("Samples").top() - rect("[3]").bottom() > 5.0);
    assert_eq!(rect("[3]").left(), rect("Samples").left());
    let dot = LEGEND_DOT_SIZE + harness.ctx.global_style().spacing.item_spacing.x;
    assert!((rect("speed").left() - rect("Samples").left() - dot).abs() < 0.5);
}

#[test]
fn columns_are_as_wide_as_their_widest_text() {
    let tooltip = Tooltip::default().group(vec![
        Row::new(
            Label::Weak("a".into()),
            [
                Value::new("1 %", Tone::Strong),
                Value::new("1 of 6", Tone::Weak),
            ],
        ),
        Row::new(
            Label::Weak("bb".into()),
            [
                Value::new("100 %", Tone::Strong),
                Value::new("10 of 60", Tone::Weak),
            ],
        ),
    ]);
    let traced = Tooltip::default().group(vec![Row::new(Label::Trace(trace("x", "x", 0)), [])]);
    Harness::new_ui(|ui| {
        let width = |text: &str| text_size(ui, text, egui::TextStyle::Body).x;
        assert_eq!(
            tooltip.widths(ui),
            [width("bb"), width("100 %"), width("10 of 60")]
        );
        let dot = LEGEND_DOT_SIZE + ui.spacing().item_spacing.x;
        assert_eq!(traced.widths(ui), [dot + width("x")]);
    })
    .run();
}

#[test]
fn tones_take_their_colours_from_the_theme() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        ctx.set_theme(theme);
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert_eq!(Tone::Faint.color(ui), ui.tokens().text_faint);
            assert_eq!(Tone::Weak.color(ui), ui.visuals().weak_text_color());
            assert_eq!(Tone::Text.color(ui), ui.visuals().text_color());
            assert_eq!(Tone::Strong.color(ui), ui.visuals().strong_text_color());
        });
    }
}

#[test]
fn missing_and_null_values_are_faint() {
    assert_eq!(Value::missing(), Value::new("—", Tone::Faint));
    let number = |number, format, unit| Value::number(number, 4, format, unit);
    let default = DisplayFormat::Default;
    assert_eq!(
        number(Number::Float(1.5), default, Some("V")),
        Value::text("1.5 V")
    );
    assert_eq!(
        number(Number::Null, default, Some("V")),
        Value::faint("null")
    );
    assert_eq!(
        number(Number::Int(255), DisplayFormat::Hex, None),
        Value::text("0xff")
    );
    assert!(Tooltip::default().group(vec![]).groups.is_empty());
}

#[test]
fn labelled_values_have_their_number_faint() {
    use crate::app::model::numbers::Number::{Null, Uint};
    let labels: Labels = [(0, "off".to_owned()), (1, "on".to_owned())].into();
    let labelled =
        |number| Value::labelled(number, 4, DisplayFormat::Labels, Some("A"), Some(&labels));
    assert_eq!(labelled(Uint(1)), [Value::text("on"), Value::faint("1 A")]);
    assert_eq!(labelled(Uint(7)), [Value::text("7 A")]);
    assert_eq!(labelled(Null), [Value::faint("null")]);
    let default = Value::labelled(Uint(1), 4, DisplayFormat::Default, None, Some(&labels));
    assert_eq!(default, [Value::text("1")]);
}

#[test]
fn the_footer_has_the_name_and_description_in_at_most_3_lines() {
    let tooltip = Tooltip::default().describe("mode", Some("Operating mode"));
    assert_eq!(
        tooltip.footer,
        Some(Footer {
            name: "mode".into(),
            description: "Operating mode".into()
        })
    );
    assert_eq!(Tooltip::default().describe("mode", None).footer, None);
    assert_eq!(Tooltip::default().describe("mode", Some("")).footer, None);

    let long = Tooltip::default().describe("mode", Some(&"word ".repeat(120)));
    let mut harness = Harness::new_ui(|ui| {
        theme::install(ui.ctx());
        let job = long.footer_job(ui, 200.0).expect("a footer");
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        assert_eq!(galley.rows.len(), 3);
        let last = galley.rows[2].row.glyphs.last().expect("glyphs");
        assert_eq!(last.chr, '…');
    });
    harness.run();

    let tooltip = Tooltip::new(Header::Text("0:00:01.000".into()))
        .group(vec![Row::new(Label::Weak("a".into()), [Value::text("1")])])
        .describe("mode", Some("Operating mode"));
    let mut harness = Harness::new_ui(|ui| {
        theme::install(ui.ctx());
        tooltip.ui(ui);
    });
    harness.run();
    let footer = harness.get_by_label("mode Operating mode");
    assert!(footer.rect().top() > harness.get_by_label("a").rect().bottom());
}
