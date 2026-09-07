//! Development-only authoring surface; production components consume its resolver.
use polyorama_ui_egui::{ApplicationTheme, ColourTokens, Rgba8, ThemeColours, ThemeVariant};

pub struct AppearanceWorkbench {
    pub open: bool,
    pub compare_reference: bool,
    pub theme: ApplicationTheme,
    colours: ThemeColours,
    preset: usize,
    error: Option<String>,
}

impl Default for AppearanceWorkbench {
    fn default() -> Self {
        let theme = ApplicationTheme::analytical();
        Self {
            open: false,
            compare_reference: false,
            colours: theme.colours(),
            theme,
            preset: 0,
            error: None,
        }
    }
}

impl AppearanceWorkbench {
    pub fn effective_theme(&self) -> ApplicationTheme {
        if self.compare_reference {
            ApplicationTheme::analytical()
        } else {
            self.theme.clone()
        }
    }

    /// Return true only for an accepted identity change. Invalid edits stay
    /// visible for repair and never replace the last valid preview.
    pub fn show(&mut self, context: &egui::Context, variant: ThemeVariant) -> bool {
        let mut changed = false;
        let mut open = self.open;
        egui::Window::new("Appearance workbench").open(&mut open).default_width(350.0).show(context, |ui| {
            ui.label("Preview authored colours on the selected production story.");
            let previous = self.preset;
            let combo = egui::ComboBox::from_id_salt("workbench.theme").selected_text(["Analytical reference", "Neutral graphite", "Warm paper"][self.preset]).show_ui(ui, |ui| {
                for (index, name) in ["Analytical reference", "Neutral graphite", "Warm paper"].iter().enumerate() {
                    let option = ui.selectable_value(&mut self.preset, index, *name);
                    polyorama_ui_egui::record_native_text_control(&option, polyorama_ui_egui::NativeTextControlKind::Selectable);
                }
            });
            polyorama_ui_egui::record_native_text_control(&combo.response, polyorama_ui_egui::NativeTextControlKind::ComboBox);
            if self.preset != previous {
                self.theme = authored_theme(self.preset);
                self.colours = self.theme.colours();
                self.error = None;
                changed = true;
            }
            changed |= ui.checkbox(&mut self.compare_reference, "Compare analytical reference").changed();
            ui.label(format!("Editing {variant:?}; mode and contrast use the gallery controls."));
            let colours = match variant {
                ThemeVariant::Light => &mut self.colours.light,
                ThemeVariant::Dark => &mut self.colours.dark,
                ThemeVariant::LightHighContrast => &mut self.colours.light_high_contrast,
                ThemeVariant::DarkHighContrast => &mut self.colours.dark_high_contrast,
            };
            let mut edited = false;
            for (name, colour) in [
                ("Canvas", &mut colours.surface_canvas),
                ("Panel", &mut colours.surface_panel),
                ("Raised", &mut colours.surface_raised),
                ("Decorative border", &mut colours.border_decorative),
                ("Control border", &mut colours.border_control),
                ("Selection background", &mut colours.selection_background),
                ("Selection marker", &mut colours.selection_indicator),
                ("Primary background", &mut colours.action_primary_background),
                ("Primary foreground", &mut colours.action_primary_foreground),
            ] {
                ui.horizontal(|ui| {
                    let mut rgb = [colour.red, colour.green, colour.blue];
                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                        *colour = Rgba8 { red: rgb[0], green: rgb[1], blue: rgb[2], alpha: 255 };
                        edited = true;
                    }
                    ui.label(name);
                });
            }
            if edited {
                match ApplicationTheme::new(self.colours) {
                    Ok(theme) => { self.theme = theme; self.error = None; changed = true; }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
            if let Some(error) = &self.error { ui.label(format!("Preview retains last valid values: {error}")); }
            let export = ui.add_enabled(self.error.is_none(), egui::Button::new("Copy theme JSON"));
            polyorama_ui_egui::record_native_text_control(&export, polyorama_ui_egui::NativeTextControlKind::Button);
            if export.clicked() {
                context.copy_text(serde_json::to_string_pretty(&self.theme.colours()).expect("typed theme serialises"));
            }
            ui.label("Export is ThemeColours JSON for checked-in application source. Snapshot baselines require separate review.");
        });
        self.open = open;
        changed
    }
}

fn rgb(value: u32) -> Rgba8 {
    Rgba8 {
        red: (value >> 16) as u8,
        green: (value >> 8) as u8,
        blue: value as u8,
        alpha: 255,
    }
}

fn authored_theme(index: usize) -> ApplicationTheme {
    if index == 0 {
        return ApplicationTheme::analytical();
    }
    let mut colours = ApplicationTheme::analytical().colours();
    let warm = index == 2;
    for (colour, dark, high) in [
        (&mut colours.light, false, false),
        (&mut colours.dark, true, false),
        (&mut colours.light_high_contrast, false, true),
        (&mut colours.dark_high_contrast, true, true),
    ] {
        author_palette(colour, dark, high, warm);
    }
    ApplicationTheme::new(colours).expect("authored gallery theme passes contrast validation")
}

fn author_palette(colour: &mut ColourTokens, dark: bool, high: bool, warm: bool) {
    let (canvas, panel, raised, selection, text, muted, marker, decorative, control) =
        if high && dark {
            (
                0x000000, 0x000000, 0x0a0a0a, 0x171717, 0xffffff, 0xdddddd, 0xffffff, 0x666666,
                0xaaaaaa,
            )
        } else if high {
            (
                0xffffff, 0xffffff, 0xfafafa, 0xe9e9e9, 0x000000, 0x222222, 0x000000, 0x999999,
                0x555555,
            )
        } else if dark {
            (
                0x131315, 0x18181b, 0x27272c, 0x303036, 0xf0f0f2, 0xbcbcc5, 0xc3c4cc, 0x333338,
                0x767680,
            )
        } else if warm {
            (
                0xf3f0e9, 0xfaf8f3, 0xffffff, 0xe3ddd2, 0x242320, 0x55514a, 0x60564a, 0xdad4ca,
                0x81796d,
            )
        } else {
            (
                0xf1f1f3, 0xf8f8fa, 0xffffff, 0xe2e2e8, 0x242428, 0x55555d, 0x565662, 0xd7d7dd,
                0x7c7c86,
            )
        };
    colour.surface_canvas = rgb(canvas);
    colour.surface_panel = rgb(panel);
    colour.surface_raised = rgb(raised);
    colour.surface_hover = rgb(selection);
    colour.selection_background = rgb(selection);
    colour.action_quiet_hover = rgb(selection);
    colour.text_primary = rgb(text);
    colour.text_muted = rgb(muted);
    colour.selection_indicator = rgb(marker);
    colour.focus_ring = rgb(if high {
        marker
    } else if dark {
        0xa7b4f8
    } else {
        0x4e569b
    });
    colour.action_primary_background = rgb(text);
    colour.action_primary_foreground = rgb(panel);
    colour.border_decorative = rgb(decorative);
    colour.border_subtle = rgb(decorative);
    colour.border_control = rgb(control);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workbench_and_production_shell_render_each_authored_identity() {
        use polyorama_ui_egui::{DensityVariant, TypographyProfile, UiPreferences};
        for index in 0..=2 {
            for variant in [
                ThemeVariant::Light,
                ThemeVariant::Dark,
                ThemeVariant::LightHighContrast,
                ThemeVariant::DarkHighContrast,
            ] {
                let theme = authored_theme(index);
                let context = egui::Context::default();
                polyorama_ui_egui::apply_design_system_with_theme(
                    &context,
                    UiPreferences::default(),
                    TypographyProfile::Dense,
                    &theme,
                );
                let tokens = theme.resolve(
                    variant,
                    DensityVariant::Comfortable,
                    TypographyProfile::Dense,
                );
                let mut workbench = AppearanceWorkbench {
                    open: true,
                    theme: theme.clone(),
                    colours: theme.colours(),
                    preset: index,
                    ..Default::default()
                };
                let mut dock =
                    crate::stories::DockSceneState::new(crate::StoryId::ReferenceApplicationShell);
                let mut observations = Vec::new();
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1100.0, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        crate::stories::render_story(
                            ui,
                            crate::StoryId::ReferenceApplicationShell,
                            &mut dock,
                            &tokens,
                            1.0,
                            &mut observations,
                            &mut Vec::new(),
                            &mut None,
                        );
                        assert!(!workbench.show(&context, variant));
                    },
                );
                output.textures_delta.clear();
                assert!(!observations.is_empty());
                assert!(polyorama_ui_egui::audit_text_layouts(&observations).is_empty());
                assert_eq!(workbench.effective_theme(), theme);
            }
        }
    }

    #[test]
    fn authored_presets_and_json_exports_are_validated() {
        for index in 0..=2 {
            let theme = authored_theme(index);
            let json = serde_json::to_string(&theme.colours()).unwrap();
            assert_eq!(
                ApplicationTheme::new(serde_json::from_str(&json).unwrap()).unwrap(),
                theme
            );
        }
    }
}
