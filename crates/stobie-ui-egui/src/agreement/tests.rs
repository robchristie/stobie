use egui::accesskit::{Action, Node, NodeId, Rect, Role, Toggled, Tree, TreeId, TreeUpdate};

use super::*;
use crate::UiRect;

fn rect() -> UiRect {
    UiRect {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 40.0,
        max_y: 20.0,
    }
}

fn semantic(id: &str, role: UiRole, name: &str) -> UiNode {
    let mut node = UiNode::container(SemanticUiId::new(id), None, role, rect());
    node.name = name.into();
    node
}

fn accesskit(id: &str, role: Role, name: &str) -> Node {
    let mut node = Node::new(role);
    node.set_label(name);
    node.set_author_id(id.to_owned());
    node.add_action(Action::Click);
    node.add_action(Action::Focus);
    node.set_bounds(Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 40.0,
        y1: 20.0,
    });
    node
}

/// A toggle, a selected tab and a focused text field, agreeing.
fn views() -> (UiSnapshot, Vec<Node>, u64) {
    let mut toggle = semantic("reviewed", UiRole::Button, "Reviewed");
    toggle.checked = Some(true);
    let mut tab = semantic("tab.1", UiRole::Tab, "Records");
    tab.selected = true;
    let mut field = semantic("title", UiRole::TextInput, "Title");
    field.focused = true;
    field.value = Some("Survey".into());
    let snapshot = UiSnapshot {
        nodes: vec![toggle, tab, field],
        ..UiSnapshot::default()
    };
    let mut toggle = accesskit("reviewed", Role::Button, "Reviewed");
    toggle.set_toggled(Toggled::True);
    let mut tab = accesskit("tab.1", Role::Tab, "Records");
    tab.set_selected(true);
    let mut field = accesskit("title", Role::TextInput, "Title");
    field.set_value("Survey");
    (snapshot, vec![toggle, tab, field], 4)
}

fn update(nodes: Vec<Node>, focus: u64) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_children((2..2 + nodes.len() as u64).map(NodeId).collect::<Vec<_>>());
    let mut all = vec![(NodeId(1), root)];
    all.extend(
        nodes
            .into_iter()
            .enumerate()
            .map(|(index, node)| (NodeId(index as u64 + 2), node)),
    );
    TreeUpdate {
        nodes: all,
        tree: Some(Tree::new(NodeId(1))),
        tree_id: TreeId::ROOT,
        focus: NodeId(focus),
    }
}

#[test]
fn agreeing_views_report_nothing() {
    let (snapshot, nodes, focus) = views();
    assert_eq!(semantic_agreement(&snapshot, &update(nodes, focus)), []);
}

#[test]
fn each_projected_field_reports_its_disagreement() {
    let fields = |snapshot: &UiSnapshot, nodes: Vec<Node>, focus| {
        semantic_agreement(snapshot, &update(nodes, focus))
            .into_iter()
            .map(|disagreement| (disagreement.id.0, disagreement.field))
            .collect::<Vec<_>>()
    };
    let (snapshot, mut nodes, focus) = views();
    nodes[0].set_label("Reviewed state");
    nodes[1].set_selected(false);
    nodes[2].set_value("Survey notes");
    assert_eq!(
        fields(&snapshot, nodes, focus),
        [
            ("reviewed".into(), AgreementField::Name),
            ("tab.1".into(), AgreementField::Selected),
            ("title".into(), AgreementField::Value),
        ]
    );
    let (snapshot, mut nodes, _) = views();
    nodes[0].set_disabled();
    nodes[0].set_toggled(Toggled::False);
    nodes[1] = accesskit("tab.1", Role::Button, "Records");
    // Focus on the toggle instead of the field.
    assert_eq!(
        fields(&snapshot, nodes, 2),
        [
            ("reviewed".into(), AgreementField::Enabled),
            ("reviewed".into(), AgreementField::Focused),
            ("reviewed".into(), AgreementField::Checked),
            ("tab.1".into(), AgreementField::Role),
            ("tab.1".into(), AgreementField::Selected),
            ("title".into(), AgreementField::Focused),
        ]
    );
    let (snapshot, mut nodes, focus) = views();
    nodes.pop();
    assert_eq!(
        fields(&snapshot, nodes, focus),
        [("title".into(), AgreementField::Missing)]
    );
}

#[test]
fn roles_the_layer_flattens_are_not_compared() {
    for role in [
        UiRole::Application,
        UiRole::Toolbar,
        UiRole::ScrollArea,
        UiRole::StatusChip,
        UiRole::Section,
    ] {
        assert_eq!(expected_aria_role(role), None);
        let snapshot = UiSnapshot {
            nodes: vec![semantic("only", role, "Only")],
            ..UiSnapshot::default()
        };
        assert_eq!(semantic_agreement(&snapshot, &update(Vec::new(), 1)), []);
    }
}
