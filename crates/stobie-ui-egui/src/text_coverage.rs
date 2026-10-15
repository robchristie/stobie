use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::{TextAuditCoverage, TextComponentId, TextExclusion, TextLayoutObservation};
use egui::{Context, Id, Response};

#[derive(Clone, Copy, Debug)]
pub enum NativeTextControlKind {
    ComboBox,
    Button,
    RadioButton,
    Slider,
    Selectable,
    /// Editable text has native cursor/selection layout, outside the measured audit.
    TextEdit,
}

impl NativeTextControlKind {
    fn exclusion(self) -> TextExclusion {
        match self {
            Self::ComboBox => TextExclusion::NativeComboBoxText,
            Self::Button => TextExclusion::NativeButtonText,
            Self::RadioButton => TextExclusion::NativeRadioButtonText,
            Self::Slider => TextExclusion::NativeSliderText,
            Self::Selectable => TextExclusion::NativeSelectableText,
            Self::TextEdit => TextExclusion::NativeTextEditText,
        }
    }
}

#[derive(Clone, Default)]
struct NativeTextInventory {
    pass: u64,
    controls: HashSet<Id>,
    exclusions: BTreeSet<TextExclusion>,
    components: BTreeMap<TextComponentId, bool>,
    /// Text edits in recording order, with their allocated rectangles.
    text_edits: Vec<(Id, egui::Rect)>,
}

fn inventory_id(context: &Context) -> Id {
    Id::new(("stobie.text-audit-coverage", context.viewport_id()))
}

/// Record a native widget without claiming its internal text was measured.
/// Call at each native text-control recipe or application call site. Responses
/// are deduplicated by egui ID, and the inventory resets on each layout pass.
pub fn record_native_text_control(response: &Response, kind: NativeTextControlKind) {
    let context = &response.ctx;
    let pass = context.cumulative_pass_nr();
    let id = inventory_id(context);
    context.data_mut(|data| {
        let inventory = data.get_temp_mut_or_default::<NativeTextInventory>(id);
        if inventory.pass != pass {
            *inventory = NativeTextInventory {
                pass,
                ..Default::default()
            };
        }
        if inventory.controls.insert(response.id) && matches!(kind, NativeTextControlKind::TextEdit)
        {
            inventory.text_edits.push((response.id, response.rect));
        }
        inventory.exclusions.insert(kind.exclusion());
    });
}

/// The egui text edits recorded in this pass, with their rectangles. Hosts
/// pair them with observed text input nodes to offer framework set text.
pub(crate) fn recorded_text_edits(context: &Context) -> Vec<(Id, egui::Rect)> {
    let pass = context.cumulative_pass_nr();
    let id = inventory_id(context);
    context.data(|data| {
        data.get_temp::<NativeTextInventory>(id)
            .filter(|inventory| inventory.pass == pass)
            .map(|inventory| inventory.text_edits)
            .unwrap_or_default()
    })
}

/// Retain outcomes independently of consumer observation filtering.
pub(crate) fn record_measured_component(
    context: &Context,
    component: TextComponentId,
    failed: bool,
) {
    let pass = context.cumulative_pass_nr();
    let id = inventory_id(context);
    context.data_mut(|data| {
        let inventory = data.get_temp_mut_or_default::<NativeTextInventory>(id);
        if inventory.pass != pass {
            *inventory = NativeTextInventory {
                pass,
                ..Default::default()
            };
        }
        inventory
            .components
            .entry(component)
            .and_modify(|previous| *previous |= failed)
            .or_insert(failed);
    });
}

/// Read coverage after rendering, inside the same UI pass as the observations.
/// An empty audit means every observed Stobie text component passed; it does
/// not establish that every visible string was structurally audited.
pub fn text_audit_coverage(
    context: &Context,
    observations: &[TextLayoutObservation],
) -> TextAuditCoverage {
    let pass = context.cumulative_pass_nr();
    let id = inventory_id(context);
    let mut coverage = TextAuditCoverage {
        measured_components: observations
            .iter()
            .map(|observation| observation.component_id)
            .collect::<BTreeSet<_>>()
            .len(),
        ..Default::default()
    };
    let failed = observations
        .iter()
        .filter(|observation| observation.layout_error.is_some())
        .map(|observation| observation.component_id)
        .collect::<BTreeSet<_>>();
    coverage.attempted_components = coverage.measured_components;
    coverage.failed_components = failed.len();
    coverage.successful_components = coverage.attempted_components - coverage.failed_components;
    context.data(|data| {
        if let Some(inventory) = data.get_temp::<NativeTextInventory>(id)
            && inventory.pass == pass
        {
            let mut outcomes = inventory.components;
            for observation in observations {
                outcomes
                    .entry(observation.component_id)
                    .and_modify(|failed| *failed |= observation.layout_error.is_some())
                    .or_insert(observation.layout_error.is_some());
            }
            coverage.attempted_components = outcomes.len();
            coverage.failed_components = outcomes.values().filter(|failed| **failed).count();
            coverage.successful_components =
                coverage.attempted_components - coverage.failed_components;
            coverage.native_text_controls = inventory.controls.len();
            coverage.excluded_categories.extend(inventory.exclusions);
        }
    });
    coverage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_inventory_deduplicates_and_does_not_leak_into_the_next_pass() {
        let context = Context::default();
        let mut output = context.run_ui(Default::default(), |ui| {
            let response = ui.radio(false, "Native option");
            record_native_text_control(&response, NativeTextControlKind::RadioButton);
            record_native_text_control(&response, NativeTextControlKind::RadioButton);
            let coverage = text_audit_coverage(ui.ctx(), &[]);
            assert_eq!(coverage.measured_components, 0);
            assert_eq!(coverage.native_text_controls, 1);
            assert_eq!(coverage.observed_native_controls, 0);
            assert_eq!(
                coverage.excluded_categories,
                vec![
                    TextExclusion::OrdinaryEguiLabels,
                    TextExclusion::NativeRadioButtonText,
                ]
            );
        });
        output.textures_delta.clear();
        let mut output = context.run_ui(Default::default(), |ui| {
            ui.label("An ordinary label is excluded, not certified");
            assert_eq!(
                text_audit_coverage(ui.ctx(), &[]),
                TextAuditCoverage::default()
            );
        });
        output.textures_delta.clear();
    }
}
