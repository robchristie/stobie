use super::*;

const FIELD: &str = "composer";

fn input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(400.0, 300.0),
        )),
        ..Default::default()
    }
}

fn run(context: &egui::Context, ui: impl FnMut(&mut Ui)) -> egui::FullOutput {
    let mut output = context.run_ui(input(), ui);
    output.textures_delta.clear();
    output
}

fn composer(context: &egui::Context, text: &mut String) -> egui::FullOutput {
    run(context, |ui| {
        NativeTextEdit::multiline(text, FIELD)
            .hint_text("Message")
            .desired_rows(1)
            .desired_width(240.0)
            .show(ui);
    })
}

/// Each painted text shape's text, with the colour of each of its sections.
fn text_colours(output: &egui::FullOutput) -> Vec<(String, Color32)> {
    output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => Some(
                text.galley
                    .job
                    .sections
                    .iter()
                    .map(|section| {
                        let colour = text.override_text_color.unwrap_or(section.format.color);
                        (text.galley.job.text.clone(), colour)
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn registers_the_field_each_pass_with_its_geometry_and_style() {
    let context = egui::Context::default();
    let mut text = "Hello".to_owned();
    composer(&context, &mut text);
    let fields = registered_in_completed_pass(&context);
    assert_eq!(fields.len(), 1);
    let field = &fields[0];
    assert_eq!(field.id, Id::new(FIELD));
    assert!(field.multiline);
    assert_eq!(field.hint, "Message");
    assert!(field.spellcheck);
    assert_eq!(field.family, NativeFont::Proportional);
    assert!(field.rect.is_positive());
    assert!(field.rect.contains_rect(field.text_rect));
    assert!(
        field.text_rect.min.x > field.rect.min.x,
        "egui's margin pads the text"
    );
    // The native field wraps where egui does, at the field's width less its
    // margin, not at the width of the text laid out.
    assert_eq!(
        field.rect.max.x - field.text_rect.max.x,
        field.text_rect.min.x - field.rect.min.x,
        "{field:?}"
    );
    assert!(field.line_height >= field.font_size);
    assert_ne!(field.text.a(), 0);
    // A pass that does not show the field registers nothing.
    run(&context, |_| {});
    assert!(registered_in_completed_pass(&context).is_empty());
}

#[test]
fn an_active_native_field_paints_no_text_hint_or_caret_blink() {
    let context = egui::Context::default();
    let mut text = "Hello".to_owned();
    let painted = composer(&context, &mut text);
    assert!(
        text_colours(&painted)
            .iter()
            .any(|(text, colour)| text == "Hello" && colour.a() > 0),
        "an inactive field draws its text"
    );

    set_native_text_active(&context, Some(Id::new(FIELD)));
    context.memory_mut(|memory| memory.request_focus(Id::new(FIELD)));
    composer(&context, &mut text);
    let active = composer(&context, &mut text);
    assert!(context.memory(|memory| memory.has_focus(Id::new(FIELD))));
    for (text, colour) in text_colours(&active) {
        assert!(
            text != "Hello" || colour == Color32::TRANSPARENT,
            "the native field draws the text, not egui ({colour:?})"
        );
    }
    // Without a blinking caret, a focused native field asks for no frames.
    assert_eq!(
        active.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
        std::time::Duration::MAX
    );

    // Empty, it draws no hint either; the native field shows its placeholder.
    let mut empty = String::new();
    let hint = composer(&context, &mut empty);
    assert!(
        text_colours(&hint)
            .iter()
            .all(|(text, colour)| text != "Message" || *colour == Color32::TRANSPARENT)
    );

    // Inactive again, egui draws the text.
    set_native_text_active(&context, None);
    let inactive = composer(&context, &mut text);
    assert!(
        text_colours(&inactive)
            .iter()
            .any(|(text, colour)| text == "Hello" && colour.a() > 0)
    );
}

#[test]
fn only_the_active_field_stops_painting() {
    let context = egui::Context::default();
    set_native_text_active(&context, Some(Id::new("other")));
    let mut text = "Hello".to_owned();
    let output = composer(&context, &mut text);
    assert!(
        text_colours(&output)
            .iter()
            .any(|(text, colour)| text == "Hello" && colour.a() > 0)
    );
}

#[test]
fn a_tap_places_the_caret_where_egui_would() {
    let context = egui::Context::default();
    let mut text = "Hello world".to_owned();
    composer(&context, &mut text);
    let field = registered_in_completed_pass(&context).remove(0);
    let row = &field.galley.rows[0];
    // Halfway into "w", the seventh character, the caret goes before or after it.
    let w = row.glyphs[6].pos.x + row.glyphs[6].advance_width * 0.25;
    let point = field.galley_pos + egui::vec2(w, row.height() / 2.0);
    assert_eq!(field.char_at(point), 6);
    // Past the end, at the end; before the start, at the start.
    assert_eq!(
        field.char_at(field.galley_pos + egui::vec2(1000.0, 1.0)),
        11
    );
    assert_eq!(field.char_at(field.galley_pos + egui::vec2(-10.0, 1.0)), 0);
}

/// A frameless field limited to `rows` lines, as a composer shows it.
fn limited(context: &egui::Context, text: &mut String, rows: usize) -> egui::FullOutput {
    run(context, |ui| {
        NativeTextEdit::multiline(text, FIELD)
            .desired_rows(1)
            .max_rows(rows)
            .frame(false)
            .margin(Margin::symmetric(8, 6))
            .desired_width(240.0)
            .show(ui);
    })
}

#[test]
fn a_limited_field_grows_to_its_rows_then_registers_the_part_shown() {
    let context = egui::Context::default();
    let mut heights = Vec::new();
    for text in ["one", "one\ntwo", "one\ntwo\nthree", "1\n2\n3\n4\n5\n6"] {
        let mut text = text.to_owned();
        // A second pass settles the scroll area's size.
        limited(&context, &mut text, 3);
        limited(&context, &mut text, 3);
        let fields = registered_in_completed_pass(&context);
        assert_eq!(fields.len(), 1);
        let field = &fields[0];
        assert!(field.rect.contains_rect(field.text_rect), "{field:?}");
        assert_eq!(
            field.text_rect.min - field.rect.min,
            egui::vec2(8.0, 6.0),
            "the margin pads the text exactly"
        );
        assert_eq!(
            field.rect.max.x - field.text_rect.max.x,
            8.0,
            "the text spans the field's width: {field:?}"
        );
        // The native field's lines fill exactly the text egui laid out.
        assert_eq!(
            field
                .galley
                .rows
                .iter()
                .map(|_| field.line_height)
                .sum::<f32>(),
            field.galley.rect.height(),
            "{field:?}"
        );
        heights.push(field.rect.height());
    }
    assert!(
        heights[0] < heights[1] && heights[1] < heights[2],
        "{heights:?}"
    );
    assert_eq!(
        heights[3], heights[2],
        "six lines show exactly three and scroll: {heights:?}"
    );
}

#[test]
fn a_frameless_field_paints_no_frame() {
    let context = egui::Context::default();
    let mut text = String::new();
    let output = limited(&context, &mut text, 3);
    assert!(
        !output.shapes.iter().any(|clipped| matches!(
            &clipped.shape,
            egui::Shape::Rect(rect) if rect.fill != Color32::TRANSPARENT
                || rect.stroke.width > 0.0
        )),
        "no fill or stroke"
    );
}

#[test]
fn a_limited_fields_hint_shows_on_one_line_elided() {
    let context = egui::Context::default();
    let mut text = String::new();
    let hint = "Message Augusta Ada King, Countess of Lovelace, about the engine";
    let mut height = |hint: &str| {
        run(&context, |ui| {
            NativeTextEdit::multiline(&mut text, FIELD)
                .hint_text(hint)
                .desired_rows(1)
                .max_rows(4)
                .desired_width(120.0)
                .show(ui);
        });
        registered_in_completed_pass(&context)[0].rect.height()
    };
    let short = height("Message");
    let long = height(hint);
    assert_eq!(long, short, "the long hint takes one line");
    assert_eq!(registered_in_completed_pass(&context)[0].hint, hint);
}
