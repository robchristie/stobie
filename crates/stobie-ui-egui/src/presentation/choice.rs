//! The segmented choice recipe: a named set of options bound to one action
//! whose argument is the option chosen.

use std::hash::Hash;

use egui::{Response, Sense, Ui, UiBuilder, vec2};

use super::PresentationContext;
use crate::components::{ButtonFace, action_button_size, present_button};
use crate::{
    ActionArgument, ActionButtonContent, ActionButtonState, ActionEmphasis, ActionKey,
    ActionTarget, Availability, BoundAction, ChoiceArgument, SemanticActionId, UiNode, UiRole,
};

/// Whether a segmented choice takes one option or any number of them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChoiceMode {
    /// One option is chosen: the options are radio buttons in a group, the
    /// chosen one checked. Tab enters the group at the chosen option and
    /// leaves it; the arrow keys, Home and End choose another.
    Single,
    /// Each option is chosen or not on its own: the options are toggle
    /// buttons, pressed while chosen, and each is a Tab stop.
    Multiple,
}

/// One option of a segmented choice: the argument its action carries, the
/// label that names it, an optional shorter label to show, and whether it is
/// chosen now.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ChoiceOption<'a> {
    /// The action's argument when this option is activated, such as the
    /// option's choice ID. Each option's must be distinct.
    pub argument: ActionArgument,
    /// The option's accessible name, shown unless a short label is given,
    /// such as "Tuesday".
    pub label: &'a str,
    /// Shown in place of the label, such as "Tue", where options must be
    /// narrow. The label stays the name and shows in the tooltip.
    pub short_label: Option<&'a str>,
    /// Whether the option is chosen now.
    pub selected: bool,
}

impl<'a> ChoiceOption<'a> {
    /// An option carrying `argument`, named and labelled `label`.
    pub fn new(argument: impl Into<ActionArgument>, label: &'a str, selected: bool) -> Self {
        Self {
            argument: argument.into(),
            label,
            short_label: None,
            selected,
        }
    }

    /// Show `short_label` in place of the label.
    pub fn with_short_label(mut self, short_label: &'a str) -> Self {
        self.short_label = Some(short_label);
        self
    }
}

impl ChoiceOption<'static> {
    /// The option for `value` of a choice argument, carrying its choice ID
    /// and named by its label from [`ChoiceArgument::CHOICES`].
    pub fn choice<C: ChoiceArgument>(value: C, selected: bool) -> Self {
        let label = C::CHOICES
            .iter()
            .find(|(option, ..)| *option == value)
            .map(|(_, _, label)| *label)
            .expect("every value is listed in CHOICES");
        Self::new(ActionArgument::choice(value), label, selected)
    }

    /// Every option of a choice argument in its order, chosen where
    /// `selected` says so.
    pub fn choices<C: ChoiceArgument>(selected: impl Fn(C) -> bool) -> Vec<Self> {
        C::CHOICES
            .iter()
            .map(|(value, ..)| Self::choice(*value, selected(*value)))
            .collect()
    }
}

/// A segmented choice: its name, its mode and its options in order.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct SegmentedChoiceSpec<'a> {
    /// The group's accessible name, such as "Repeats". Show it as the
    /// choice's heading or field label too.
    pub name: &'a str,
    pub mode: ChoiceMode,
    pub options: &'a [ChoiceOption<'a>],
}

impl<'a> SegmentedChoiceSpec<'a> {
    /// A single choice named `name`. At most one option should be selected.
    pub fn single(name: &'a str, options: &'a [ChoiceOption<'a>]) -> Self {
        Self {
            name,
            mode: ChoiceMode::Single,
            options,
        }
    }

    /// A multiple choice named `name`.
    pub fn multiple(name: &'a str, options: &'a [ChoiceOption<'a>]) -> Self {
        Self {
            name,
            mode: ChoiceMode::Multiple,
            options,
        }
    }
}

/// What a segmented choice presented and which option was activated.
#[derive(Debug)]
pub struct SegmentedChoiceResponse {
    /// The whole group.
    pub response: Response,
    /// Each option's response, in the specification's order; a hidden
    /// option's is an empty hover response.
    pub options: Vec<Response>,
    /// The option activated in this pass, by its index in the
    /// specification: clicked, or, in a single choice, moved to with the
    /// keyboard. Perform the action with its argument.
    pub activated: Option<usize>,
}

impl SegmentedChoiceResponse {
    /// The activated option's argument, from the specification presented.
    pub fn activated_argument<'s>(
        &self,
        spec: &'s SegmentedChoiceSpec<'_>,
    ) -> Option<&'s ActionArgument> {
        self.activated.map(|index| &spec.options[index].argument)
    }
}

/// The options' layout: each option as wide as the widest, in balanced rows
/// that each fit the available width.
fn rows(count: usize, cell: f32, gap: f32, available: f32) -> usize {
    if count == 0 {
        return 0;
    }
    let per_row = (((available + gap) / (cell + gap)).floor() as usize).clamp(1, count);
    let rows = count.div_ceil(per_row);
    count.div_ceil(rows)
}

/// The next enabled option from `from`, stepping by `step` and wrapping.
fn step_enabled(enabled: &[bool], from: usize, forward: bool) -> Option<usize> {
    let count = enabled.len();
    (1..=count)
        .map(|offset| {
            if forward {
                (from + offset) % count
            } else {
                (from + count - offset % count) % count
            }
        })
        .find(|&index| enabled[index])
}

impl PresentationContext {
    /// Present a segmented choice: `spec`'s options side by side, each as
    /// wide as the widest, wrapping into balanced rows where the width is
    /// short. Activating an option performs `target` with that option's
    /// argument, such as its choice ID; `availability` gives each option's
    /// availability for its argument, and a hidden option presents nothing.
    ///
    /// A single choice publishes a [`UiRole::Section`] node named by
    /// `spec.name`, an AccessKit radio group, holding one
    /// [`UiRole::RadioButton`] per option, checked while chosen. Tab enters
    /// the group at the chosen option, or the first enabled one, and leaves
    /// it; the arrow keys move to the previous or next enabled option,
    /// wrapping, and Home and End to the first and last, each choosing the
    /// option it reaches, as Space and clicking do. A multiple choice
    /// publishes a group of [`UiRole::Button`] toggles, each pressed while
    /// chosen and each a Tab stop. Each option's node carries its argument,
    /// so agents see one control per option.
    ///
    /// Options are named by their labels and show their short labels where
    /// given, with the label in the tooltip. The choice never changes its
    /// own state: the application performs the action and presents the new
    /// selection on a later pass. Hosted applications use
    /// [`Frame::segmented_choice`](crate::Frame::segmented_choice), which
    /// triggers the action.
    ///
    /// ```
    /// use stobie_ui_egui::{ChoiceOption, PresentationContext, SegmentedChoiceSpec};
    /// # use stobie_ui_egui::{ActionKey, ActionScope, ActionSpec, ActionTarget, Availability};
    /// # #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
    /// # enum Act { Weekday }
    /// # impl ActionKey for Act {
    /// #     fn stable_id(self) -> &'static str { "routine.weekday" }
    /// #     fn specification(self) -> ActionSpec<Self> {
    /// #         ActionSpec { id: self, label: "Weekday", description: "Run on this weekday",
    /// #             compact_label: None, shortcut: None, scope: ActionScope::Application }
    /// #     }
    /// # }
    /// fn weekdays(ui: &mut egui::Ui, p: &mut PresentationContext, on: [bool; 7]) -> Option<usize> {
    ///     let names = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    ///     let options: Vec<_> = names
    ///         .iter()
    ///         .zip(on)
    ///         .map(|(name, on)| ChoiceOption::new(*name, name, on).with_short_label(&name[..3]))
    ///         .collect();
    ///     let spec = SegmentedChoiceSpec::multiple("Runs on", &options);
    ///     let target = ActionTarget::application(Act::Weekday);
    ///     p.segmented_choice(ui, "weekdays", target, &spec, |_, _| Availability::Enabled)
    ///         .activated
    /// }
    /// ```
    pub fn segmented_choice<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        target: ActionTarget<A>,
        spec: &SegmentedChoiceSpec<'_>,
        availability: impl Fn(ActionTarget<A>, &ActionArgument) -> Availability,
    ) -> SegmentedChoiceResponse {
        self.check_pass(ui);
        let group = self.scope.instance(key);
        let single = spec.mode == ChoiceMode::Single;
        let action = target.action.specification();
        let tokens = self.tokens;
        let font_scale = self.font_scale;
        let options: Vec<_> = spec
            .options
            .iter()
            .map(|option| {
                debug_assert!(
                    option.argument != ActionArgument::None,
                    "each option carries an argument"
                );
                let value = option.argument.to_value();
                let identity = group
                    .child((
                        "stobie.argument",
                        value.as_ref().map(ToString::to_string).unwrap_or_default(),
                    ))
                    .action_identity();
                let mut bound = BoundAction::for_target(target, option.argument.clone())
                    .with_label(option.label);
                if let Some(short_label) = option.short_label {
                    bound = bound.with_short_label(short_label);
                }
                let availability = availability(target, &option.argument);
                (bound, identity, availability, value)
            })
            .collect();
        let shown: Vec<usize> = (0..options.len())
            .filter(|&index| options[index].2.visible())
            .collect();
        let enabled: Vec<bool> = shown
            .iter()
            .map(|&index| options[index].2.enabled())
            .collect();

        // A single choice keeps one option a Tab stop: the focused one, else
        // the chosen one, else the first enabled one.
        let focused = shown
            .iter()
            .position(|&index| ui.memory(|memory| memory.has_focus(options[index].1.widget_id)));
        let mut activated = None;
        let mut roving = None;
        if single {
            roving = focused
                .filter(|&position| enabled[position])
                .or_else(|| {
                    shown.iter().enumerate().position(|(position, &index)| {
                        enabled[position] && spec.options[index].selected
                    })
                })
                .or_else(|| enabled.iter().position(|&enabled| enabled));
            if let Some(position) = focused.filter(|&position| enabled[position]) {
                let id = options[shown[position]].1.widget_id;
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        id,
                        egui::EventFilter {
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            ..Default::default()
                        },
                    );
                });
                let moved = ui.input_mut(|input| {
                    use egui::{Key, Modifiers};
                    if input.consume_key(Modifiers::NONE, Key::ArrowLeft)
                        || input.consume_key(Modifiers::NONE, Key::ArrowUp)
                    {
                        step_enabled(&enabled, position, false)
                    } else if input.consume_key(Modifiers::NONE, Key::ArrowRight)
                        || input.consume_key(Modifiers::NONE, Key::ArrowDown)
                    {
                        step_enabled(&enabled, position, true)
                    } else if input.consume_key(Modifiers::NONE, Key::Home) {
                        enabled.iter().position(|&enabled| enabled)
                    } else if input.consume_key(Modifiers::NONE, Key::End) {
                        enabled.iter().rposition(|&enabled| enabled)
                    } else {
                        None
                    }
                });
                if let Some(next) = moved {
                    // The group owns these keys: cancel any spatial focus move
                    // egui queued for them.
                    ui.memory_mut(|memory| {
                        memory.move_focus(egui::FocusDirection::None);
                        memory.request_focus(options[shown[next]].1.widget_id);
                    });
                    roving = Some(next);
                    activated = Some(shown[next]);
                }
            }
        }

        let visible_labels: Vec<&str> = shown
            .iter()
            .map(|&index| options[index].0.visible_label())
            .collect();
        let cell = visible_labels
            .iter()
            .map(|label| {
                let size =
                    action_button_size(ui, label, ActionButtonContent::Text, &tokens, font_scale);
                size.intrinsic.max(size.minimum_width)
            })
            .fold(tokens.geometry.minimum_hit_size.0, f32::max);
        let gap = tokens.spacing.unit.0;
        let per_row = rows(shown.len(), cell, gap, ui.available_width());

        let mut responses: Vec<Option<Response>> = vec![None; options.len()];
        let mut nodes = Vec::with_capacity(shown.len());
        let group_semantic = group.semantic_id();
        let scope = ui.scope_builder(UiBuilder::new().id_salt(group.egui_id()), |ui| {
            ui.spacing_mut().item_spacing = vec2(gap, gap);
            for (row, chunk) in shown.chunks(per_row.max(1)).enumerate() {
                ui.horizontal(|ui| {
                    for (offset, &index) in chunk.iter().enumerate() {
                        let position = row * per_row + offset;
                        let option = &spec.options[index];
                        let (bound, identity, availability, value) = &options[index];
                        let name = bound.accessible_name();
                        let visible_label = bound.visible_label();
                        let face = ButtonFace {
                            name: &name,
                            visible_label,
                            description: Some(action.description),
                            shortcut: None,
                            availability,
                            state: ActionButtonState::Toggle {
                                pressed: option.selected,
                            },
                            emphasis: ActionEmphasis::Normal,
                            content: ActionButtonContent::Text,
                            also_fits: &visible_labels,
                            tooltip: visible_label != name,
                            tab_stop: !single || roving == Some(position),
                        };
                        let response = present_button(
                            ui,
                            &face,
                            identity,
                            &tokens,
                            font_scale,
                            &mut self.text_layouts,
                        );
                        if response.interact_rect.is_positive() {
                            ui.ctx().accesskit_node_builder(response.id, |node| {
                                if single {
                                    node.set_role(egui::accesskit::Role::RadioButton);
                                }
                                node.set_position_in_set(position + 1);
                                node.set_size_of_set(shown.len());
                            });
                        }
                        if response.clicked() {
                            activated = Some(index);
                        }
                        if response.interact_rect.is_positive() {
                            let mut node = UiNode::container(
                                identity.semantic_id.clone(),
                                Some(group_semantic.clone()),
                                if single {
                                    UiRole::RadioButton
                                } else {
                                    UiRole::Button
                                },
                                response.interact_rect.into(),
                            );
                            node.name = name.clone();
                            node.description = Some(action.description.to_owned());
                            node.enabled = availability.enabled();
                            node.focused = response.has_focus();
                            node.checked = Some(option.selected);
                            node.pane = target.pane.or(self.pane);
                            node.domain_reference = self
                                .domain_reference
                                .clone()
                                .or(target.pane.map(crate::DomainReference::Pane));
                            node.actions = vec![SemanticActionId::from_action(target.action)];
                            node.disabled_reason =
                                availability.disabled_reason().map(ToOwned::to_owned);
                            node.argument = value.clone();
                            nodes.push((node, ui.clip_rect()));
                        }
                        responses[index] = Some(response);
                    }
                });
            }
        });
        let bounds = scope.response.rect.intersect(ui.clip_rect());
        ui.ctx().accesskit_node_builder(scope.response.id, |node| {
            node.set_role(if single {
                egui::accesskit::Role::RadioGroup
            } else {
                egui::accesskit::Role::Group
            });
            node.set_label(spec.name);
            node.set_author_id(group_semantic.0.clone());
            if bounds.is_positive() {
                node.set_bounds(egui::accesskit::Rect {
                    x0: f64::from(bounds.min.x),
                    y0: f64::from(bounds.min.y),
                    x1: f64::from(bounds.max.x),
                    y1: f64::from(bounds.max.y),
                });
            }
        });
        if !shown.is_empty() && scope.response.rect.is_positive() {
            // The group's node leads its options' nodes, which it holds.
            let mut node = UiNode::container(
                group_semantic,
                Some(self.parent.clone()),
                UiRole::Section,
                scope.response.rect.into(),
            );
            node.name = spec.name.to_owned();
            node.domain_reference = self.domain_reference.clone();
            self.push_node(ui, node);
            for (node, clip) in nodes {
                self.semantic_nodes.push(node);
                self.node_clips.push(clip);
            }
        }
        let options = responses
            .into_iter()
            .zip(&options)
            .map(|(response, (_, identity, ..))| {
                response.unwrap_or_else(|| {
                    ui.interact(egui::Rect::NOTHING, identity.widget_id, Sense::hover())
                })
            })
            .collect();
        SegmentedChoiceResponse {
            response: scope.response,
            options,
            activated,
        }
    }
}

#[cfg(test)]
#[path = "choice_tests.rs"]
mod tests;
