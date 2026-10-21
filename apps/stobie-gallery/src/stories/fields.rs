use eframe::egui;
use stobie_ui_egui::{
    DesignTokens, PresentationContext, PresentationScope, SemanticUiId, TextFieldSpec,
    observe_visible_only,
};

use super::StoryReport;

/// The field IDs, which agents use with `stobie set-text`.
pub(crate) const NAME_FIELD: &str = "gallery.join.name";
pub(crate) const CODE_FIELD: &str = "gallery.join.code";
pub(crate) const PASSWORD_FIELD: &str = "gallery.join.password";
pub(crate) const ABOUT_FIELD: &str = "gallery.join.about";
pub(crate) const ACCOUNT_FIELD: &str = "gallery.join.account";

const CODE_ERROR: &str = "An invitation code has six letters or digits.";

/// Consumer-owned form state: what each field holds.
#[derive(Clone, Debug)]
pub struct FieldFixtureState {
    pub name: String,
    pub code: String,
    pub password: String,
    pub about: String,
    pub submitted: u32,
}

impl Default for FieldFixtureState {
    fn default() -> Self {
        Self {
            name: String::new(),
            code: "QX7-44".into(),
            password: "correct horse".into(),
            about: String::new(),
            submitted: 0,
        }
    }
}

/// A sign-up form's text fields: an empty field showing its hint, a field
/// with a description and an error, a password, a multiline field, and a
/// disabled one. The page scrolls, so it publishes only what is visible.
pub(super) fn story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut FieldFixtureState,
    report: &mut StoryReport,
) {
    observe_visible_only(ui, |ui| form(ui, tokens, scale, state, report));
}

fn form(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut FieldFixtureState,
    report: &mut StoryReport,
) {
    let mut p = PresentationContext::new(
        ui,
        *tokens,
        scale,
        PresentationScope::new("gallery.join"),
        SemanticUiId::new("gallery.story"),
    );
    let gap = tokens.spacing.block.0;
    let mut changed = false;
    let mut submitted = false;
    egui::ScrollArea::vertical()
        .id_salt("fields-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let name = TextFieldSpec::new("Display name")
                .id(NAME_FIELD)
                .hint("How others see you");
            let response = p.text_field(ui, "name", &name, &mut state.name);
            changed |= response.changed;
            submitted |= response.submitted;
            ui.add_space(gap);
            let code = TextFieldSpec::new("Invitation code")
                .id(CODE_FIELD)
                .spellcheck(false)
                .description("Six letters or digits from the invitation.")
                .error((code_length(&state.code) != 6).then_some(CODE_ERROR));
            let response = p.text_field(ui, "code", &code, &mut state.code);
            changed |= response.changed;
            submitted |= response.submitted;
            ui.add_space(gap);
            let password = TextFieldSpec::new("Password").id(PASSWORD_FIELD).password();
            let response = p.text_field(ui, "password", &password, &mut state.password);
            changed |= response.changed;
            submitted |= response.submitted;
            ui.add_space(gap);
            let about = TextFieldSpec::new("About you")
                .id(ABOUT_FIELD)
                .multiline(3)
                .hint("A sentence or two for the people you join")
                .description("Return starts a new line.");
            changed |= p.text_field(ui, "about", &about, &mut state.about).changed;
            ui.add_space(gap);
            ui.add_enabled_ui(false, |ui| {
                let account = TextFieldSpec::new("Account number")
                    .id(ACCOUNT_FIELD)
                    .description("Assigned when you join; it cannot be changed.");
                let mut account_number = String::from("40 2215 0093");
                p.text_field(ui, "account", &account, &mut account_number);
            });
        });
    if submitted {
        state.submitted += 1;
    }
    if changed || submitted {
        report.repaint = Some("form changed");
    }
    report.presented(&p.finish(ui));
}

fn code_length(code: &str) -> usize {
    code.chars().filter(char::is_ascii_alphanumeric).count()
}

#[cfg(test)]
mod tests {
    use stobie_ui_egui::{
        ApplicationTheme, DensityVariant, ThemeVariant, UiRole, audit_accesskit, audit_text_layouts,
    };

    use super::*;
    use crate::catalogue::StoryId;
    use crate::stories::probe::{StoryProbe, render};

    fn probe(
        variant: ThemeVariant,
        density: DensityVariant,
        scale: f32,
        size: egui::Vec2,
    ) -> (stobie_ui_egui::UiSnapshot, egui::FullOutput) {
        let context = egui::Context::default();
        context.enable_accesskit();
        render(
            &context,
            StoryProbe {
                story: StoryId::FormTextFields,
                tokens: DesignTokens::resolve(variant, density),
                scale,
                clip: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                theme: ApplicationTheme::analytical(),
                preferences: Default::default(),
                heading: false,
            },
            size,
        )
    }

    #[test]
    fn the_fields_are_named_by_their_captions_and_the_password_publishes_nothing() {
        let (snapshot, mut output) = probe(
            ThemeVariant::Dark,
            DensityVariant::Comfortable,
            1.0,
            egui::vec2(616.0, 1200.0),
        );
        assert!(snapshot.text_audit.is_empty(), "{:#?}", snapshot.text_audit);
        let fields: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| matches!(node.role, UiRole::TextInput | UiRole::MultilineTextInput))
            .map(|node| (node.id.0.as_str(), node.name.as_str(), node.enabled))
            .collect();
        assert_eq!(
            fields,
            [
                (NAME_FIELD, "Display name", true),
                (CODE_FIELD, "Invitation code", true),
                (PASSWORD_FIELD, "Password", true),
                (ABOUT_FIELD, "About you", true),
                (ACCOUNT_FIELD, "Account number", false),
            ]
        );
        let code = snapshot.node(&SemanticUiId::new(CODE_FIELD)).unwrap();
        assert_eq!(
            code.description.as_deref(),
            Some(
                "An invitation code has six letters or digits.\nSix letters or digits from the invitation."
            )
        );
        // The error is announced as a status.
        assert!(
            snapshot
                .nodes
                .iter()
                .any(|node| node.role == UiRole::Status && node.name == CODE_ERROR)
        );
        let update = output.platform_output.accesskit_update.take().unwrap();
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{findings:?}");
        // The password is never in the snapshot or the accessibility tree.
        let published = serde_json::to_string(&snapshot).unwrap();
        assert!(!published.contains("correct horse"));
        assert!(!format!("{update:?}").contains("correct horse"));
        output.textures_delta.clear();
    }

    #[test]
    fn a_narrow_column_at_larger_text_keeps_every_line_whole() {
        let (snapshot, mut output) = probe(
            ThemeVariant::LightHighContrast,
            DensityVariant::Comfortable,
            1.5,
            egui::vec2(296.0, 1600.0),
        );
        let findings = audit_text_layouts(&snapshot.text);
        assert!(findings.is_empty(), "{findings:#?}");
        let update = output.platform_output.accesskit_update.take().unwrap();
        assert!(audit_accesskit(&snapshot, &update).is_empty());
        output.textures_delta.clear();
    }
}
