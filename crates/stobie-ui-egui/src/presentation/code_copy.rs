//! Markdown whose code blocks each carry a copy control.

use std::hash::Hash;

use egui::{Response, Ui, UiBuilder};

use super::PresentationContext;
use crate::components::{icon_action_side, present_markdown};
use crate::{ActionEmphasis, Availability, CopyContent, CopySpec, MarkdownSpec};

/// The words and face of the copy control on each code block in
/// [`PresentationContext::markdown_with_code_copy`]. The application
/// supplies every word, so it owns their language.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodeCopySpec<'a> {
    label: &'a str,
    confirmation: &'a str,
    failure: &'a str,
    description: Option<&'a str>,
    content: CopyContent,
}

impl<'a> CodeCopySpec<'a> {
    /// Controls named `label`, such as "Copy code", showing their icon, and
    /// showing and announcing `confirmation` once copied, such as "Copied",
    /// or `failure` when the platform refuses, such as "Couldn't copy".
    pub const fn new(label: &'a str, confirmation: &'a str, failure: &'a str) -> Self {
        Self {
            label,
            confirmation,
            failure,
            description: None,
            content: CopyContent::Icon,
        }
    }

    /// A description for assistive technology and the tooltip.
    #[must_use]
    pub const fn description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Show the label, or the icon and the label, instead of the icon alone.
    #[must_use]
    pub const fn content(mut self, content: CopyContent) -> Self {
        self.content = content;
        self
    }

    /// The accessible name of the control on the code block at `index`, in
    /// document order counting from 0: the label alone for the first block,
    /// and the label and the block's number from 2 for each later one, so
    /// each name is distinct. The name never depends on how many blocks
    /// there are (`count`, kept for compatibility), so a control keeps its
    /// name as later blocks of a reply still arriving appear.
    pub fn name(&self, index: usize, count: usize) -> String {
        let _ = count;
        match index {
            0 => self.label.to_owned(),
            _ => format!("{} {}", self.label, index + 1),
        }
    }
}

impl PresentationContext {
    /// [`Self::markdown`] with a copy control ([`Self::copy`]) on each code
    /// block, at the trailing end of a strip across the block's top, inside
    /// its surface. Each control copies exactly the block's text as the
    /// Markdown parses it: without its fences, its language or its final
    /// line break, and with its tabs, which the block shows as four spaces,
    /// so a copied Makefile still works. Its name is
    /// [`CodeCopySpec::name`]: "Copy code" for the first block and "Copy
    /// code 2" for the second. Controls are keyed by the block's position
    /// among the code blocks, so a reply still arriving keeps each control,
    /// its name and any confirmation it shows, as later blocks arrive.
    ///
    /// ```
    /// use stobie_ui_egui::{CodeCopySpec, DesignTokens, MarkdownSpec,
    ///     PresentationContext, PresentationObservations, PresentationScope,
    ///     SemanticUiId};
    ///
    /// fn reply(ui: &mut egui::Ui, tokens: DesignTokens, scale: f32,
    ///          source: &str) -> PresentationObservations {
    ///     let mut p = PresentationContext::new(ui, tokens, scale,
    ///         PresentationScope::new("reply"), SemanticUiId::root());
    ///     p.markdown_with_code_copy(ui, "body", source, MarkdownSpec::default(),
    ///         &CodeCopySpec::new("Copy code", "Copied", "Couldn't copy"));
    ///     p.finish(ui)
    /// }
    /// ```
    pub fn markdown_with_code_copy(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        source: &str,
        spec: MarkdownSpec<'_>,
        copy: &CodeCopySpec<'_>,
    ) -> Response {
        self.check_pass(ui);
        let header = icon_action_side(&self.tokens, self.font_scale);
        let (presented, blocks) = present_markdown(
            ui,
            self.scope.instance(&key),
            source,
            spec,
            Some(header),
            self.parent.clone(),
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        if self.followed.is_none() {
            self.followed = presented.followed;
        }
        for mut node in presented
            .link_nodes
            .into_iter()
            .chain(presented.heading_nodes)
            .chain(presented.task_nodes)
            .chain(presented.footnote_nodes)
        {
            node.domain_reference = self.domain_reference.clone();
            self.push_node(ui, node);
        }
        let inset = self.tokens.spacing.unit.0;
        for (index, block) in blocks.iter().enumerate() {
            let name = copy.name(index, blocks.len());
            let strip = egui::Rect::from_min_max(
                block.header.min,
                egui::pos2(
                    (block.header.max.x - inset).max(block.header.min.x),
                    block.header.max.y,
                ),
            );
            let mut strip = ui.new_child(
                UiBuilder::new()
                    .max_rect(strip)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            self.copy(
                &mut strip,
                (&key, "stobie.code-copy", index),
                &CopySpec {
                    text: &block.text,
                    label: &name,
                    description: copy.description,
                    confirmation: copy.confirmation,
                    failure: copy.failure,
                    content: copy.content,
                    emphasis: ActionEmphasis::QuietBorderless,
                    availability: Availability::Enabled,
                },
            );
        }
        presented.response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_block_is_named_by_the_label_and_later_ones_by_their_numbers() {
        let spec = CodeCopySpec::new("Copy code", "Copied", "Couldn't copy");
        assert_eq!(spec.name(0, 1), "Copy code");
        let names: Vec<_> = (0..3).map(|index| spec.name(index, 3)).collect();
        assert_eq!(names, ["Copy code", "Copy code 2", "Copy code 3"]);
        // However many blocks there are, a block keeps its name.
        assert!((1..5).all(|count| spec.name(0, count) == "Copy code"));
    }
}
