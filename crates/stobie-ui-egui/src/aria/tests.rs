use egui::accesskit::{Action, Live, Node, NodeId, Rect, Role, Toggled, Tree, TreeId, TreeUpdate};

use super::*;

fn node(role: Role, children: &[u64]) -> Node {
    let mut node = Node::new(role);
    node.set_children(children.iter().map(|id| NodeId(*id)).collect::<Vec<_>>());
    node
}

fn bounds(node: &mut Node, x0: f64, y0: f64, x1: f64, y1: f64) {
    node.set_bounds(Rect { x0, y0, x1, y1 });
}

fn update(nodes: Vec<(u64, Node)>, focus: u64) -> TreeUpdate {
    TreeUpdate {
        nodes: nodes
            .into_iter()
            .map(|(id, node)| (NodeId(id), node))
            .collect(),
        tree: Some(Tree::new(NodeId(1))),
        tree_id: TreeId::ROOT,
        focus: NodeId(focus),
    }
}

/// A window with an unnamed container holding a named pane, which holds a
/// button, a label and a status line.
fn sample(button_label: &str, focus: u64) -> TreeUpdate {
    let mut pane = node(Role::Pane, &[4, 5, 6]);
    pane.set_label("Detail");
    pane.set_author_id("pane.2");
    bounds(&mut pane, 10.0, 20.0, 210.0, 220.0);
    let mut button = node(Role::Button, &[]);
    button.set_label(button_label);
    button.set_author_id("record-desk.apply");
    button.add_action(Action::Click);
    button.add_action(Action::Focus);
    bounds(&mut button, 20.0, 30.0, 80.0, 54.0);
    let mut label = node(Role::Label, &[]);
    label.set_value("Committed record");
    bounds(&mut label, 20.0, 60.0, 120.0, 76.0);
    let mut status = node(Role::Status, &[]);
    status.set_value("Saved");
    status.set_live(Live::Polite);
    status.set_author_id("status");
    bounds(&mut status, 20.0, 80.0, 120.0, 96.0);
    let mut container = node(Role::GenericContainer, &[3]);
    bounds(&mut container, 0.0, 0.0, 400.0, 300.0);
    update(
        vec![
            (1, node(Role::Window, &[2])),
            (2, container),
            (3, pane),
            (4, button),
            (5, label),
            (6, status),
        ],
        focus,
    )
}

#[test]
fn projection_flattens_layout_and_maps_roles_names_and_geometry() {
    let tree = project(&sample("Apply", 1), 1.0);
    assert_eq!(tree.roots, ["stobie-a11y:pane.2"]);
    let pane = &tree.elements["stobie-a11y:pane.2"];
    assert_eq!(pane.parent, None);
    assert_eq!(pane.attributes["role"], "region");
    assert_eq!(pane.attributes["aria-label"], "Detail");
    assert_eq!(
        pane.style,
        "left:10.0px;top:20.0px;width:200.0px;height:200.0px"
    );
    assert_eq!(pane.children.len(), 3);

    let button = &tree.elements["stobie-a11y:record-desk.apply"];
    assert_eq!(button.parent.as_deref(), Some("stobie-a11y:pane.2"));
    assert_eq!(
        button.attributes["tabindex"], "-1",
        "focusable controls take focus"
    );
    assert_eq!(button.node, NodeId(4));
    assert_eq!(button.attributes["role"], "button");
    assert_eq!(button.attributes["aria-label"], "Apply");
    assert_eq!(button.text, None);
    // Children are positioned within their parent element.
    assert_eq!(
        button.style,
        "left:10.0px;top:10.0px;width:60.0px;height:24.0px"
    );

    let label = &tree.elements["stobie-a11y:egui-5"];
    assert!(!label.attributes.contains_key("role"));
    assert!(!label.attributes.contains_key("tabindex"));
    assert_eq!(label.text.as_deref(), Some("Committed record"));

    // A status is implicitly a polite live region.
    let status = &tree.elements["stobie-a11y:status"];
    assert_eq!(status.attributes["role"], "status");
    assert!(!status.attributes.contains_key("aria-live"));
    assert_eq!(status.text.as_deref(), Some("Saved"));
    assert_eq!(tree.active, None);
}

#[test]
fn projection_scales_points_to_css_pixels_and_names_the_focused_element() {
    let tree = project(&sample("Apply", 4), 2.0);
    assert_eq!(
        tree.elements["stobie-a11y:record-desk.apply"].style,
        "left:20.0px;top:20.0px;width:120.0px;height:48.0px"
    );
    assert_eq!(
        tree.active.as_deref(),
        Some("stobie-a11y:record-desk.apply")
    );
}

#[test]
fn projection_maps_states_values_and_relations() {
    let mut splitter = node(Role::Splitter, &[]);
    splitter.set_label("Vertical splitter");
    splitter.set_numeric_value(0.5);
    splitter.set_min_numeric_value(0.1);
    splitter.set_max_numeric_value(0.9);
    splitter.set_orientation(egui::accesskit::Orientation::Vertical);
    let mut tabs = node(Role::TabList, &[4]);
    tabs.set_label("Workspace panes");
    let mut tab = node(Role::Tab, &[]);
    tab.set_label("Records");
    tab.set_selected(true);
    let mut toggle = node(Role::Button, &[]);
    toggle.set_label("Reviewed");
    toggle.set_toggled(Toggled::True);
    let mut disabled = node(Role::Button, &[]);
    disabled.set_label("Undo");
    disabled.set_disabled();
    disabled.set_description("Reverse the last change; unavailable: Nothing to undo");
    let mut field = node(Role::TextInput, &[9]);
    field.set_label("Record title");
    field.set_value("Survey notes");
    field.push_labelled_by(NodeId(10));
    let mut heading = node(Role::Heading, &[]);
    heading.set_value("Record detail");
    heading.set_level(2);
    let mut caption = node(Role::Label, &[]);
    caption.set_value("Title");
    let mut combo = node(Role::ComboBox, &[]);
    combo.set_label("Category filter");
    combo.set_value("All categories");
    let mut hidden = node(Role::Button, &[]);
    hidden.set_label("Hidden");
    hidden.set_hidden();
    let tree = project(
        &update(
            vec![
                (1, node(Role::Window, &[2, 3, 5, 6, 7, 8, 10, 11, 12])),
                (2, splitter),
                (3, tabs),
                (4, tab),
                (5, toggle),
                (6, disabled),
                (7, heading),
                (8, field),
                (9, node(Role::TextRun, &[])),
                (10, caption),
                (11, hidden),
                (12, combo),
            ],
            1,
        ),
        1.0,
    );
    let attributes = |id: u64| &tree.elements[&format!("stobie-a11y:egui-{id:x}")].attributes;
    assert_eq!(attributes(2)["role"], "separator");
    assert_eq!(attributes(2)["aria-valuenow"], "0.5");
    assert_eq!(attributes(2)["aria-valuemin"], "0.1");
    assert_eq!(attributes(2)["aria-valuemax"], "0.9");
    assert_eq!(attributes(2)["aria-orientation"], "vertical");
    assert_eq!(attributes(3)["role"], "tablist");
    assert_eq!(
        tree.elements["stobie-a11y:egui-3"].children,
        ["stobie-a11y:egui-4"]
    );
    assert_eq!(attributes(4)["aria-selected"], "true");
    assert_eq!(attributes(5)["aria-pressed"], "true");
    assert_eq!(attributes(6)["aria-disabled"], "true");
    assert_eq!(
        attributes(6)["aria-description"],
        "Reverse the last change; unavailable: Nothing to undo"
    );
    assert_eq!(attributes(7)["role"], "heading");
    assert_eq!(attributes(7)["aria-level"], "2");
    // A text field is a native input, whose value browsers expose.
    assert!(!attributes(8).contains_key("role"));
    assert_eq!(attributes(8)["aria-labelledby"], "stobie-a11y:egui-a");
    let field = &tree.elements["stobie-a11y:egui-8"];
    assert_eq!(field.tag, "input");
    assert_eq!(field.attributes["tabindex"], "-1");
    assert_eq!(field.attributes["autocomplete"], "off");
    assert_eq!(field.attributes["spellcheck"], "false");
    assert_eq!(field.text.as_deref(), Some("Survey notes"));
    assert!(
        field.children.is_empty(),
        "text runs belong to their control"
    );
    assert!(!tree.elements.contains_key("stobie-a11y:egui-b"));
    assert_eq!(attributes(12)["role"], "combobox");
    assert_eq!(attributes(12)["aria-expanded"], "false");
    assert_eq!(
        tree.elements["stobie-a11y:egui-c"].text.as_deref(),
        Some("All categories")
    );
}

#[test]
fn duplicate_author_ids_fall_back_to_node_ids() {
    let mut first = node(Role::Button, &[]);
    first.set_author_id("same");
    let mut second = node(Role::Button, &[]);
    second.set_author_id("same");
    let tree = project(
        &update(
            vec![(1, node(Role::Window, &[2, 3])), (2, first), (3, second)],
            1,
        ),
        1.0,
    );
    assert_eq!(tree.roots, ["stobie-a11y:same", "stobie-a11y:egui-3"]);
}

#[test]
fn an_unchanged_tree_needs_no_dom_operation() {
    let first = project(&sample("Apply", 1), 1.0);
    let initial = diff(&AriaTree::default(), &first);
    assert!(initial.contains(&DomOp::Create {
        id: "stobie-a11y:pane.2".into(),
        tag: "div",
    }));
    assert!(initial.contains(&DomOp::Place {
        id: "stobie-a11y:pane.2".into(),
        parent: None,
        before: None,
    }));
    assert!(diff(&first, &project(&sample("Apply", 1), 1.0)).is_empty());
}

#[test]
fn a_change_writes_only_what_changed() {
    let before = project(&sample("Apply", 1), 1.0);
    let after = project(&sample("Apply draft", 4), 1.0);
    assert_eq!(
        diff(&before, &after),
        [
            DomOp::SetAttribute {
                id: "stobie-a11y:record-desk.apply".into(),
                name: "aria-label",
                value: "Apply draft".into(),
            },
            DomOp::Active {
                id: Some("stobie-a11y:record-desk.apply".into()),
            },
        ]
    );
}

#[test]
fn reordering_moves_only_what_left_its_relative_order() {
    let before = project(&sample("Apply", 1), 1.0);
    let mut reordered = sample("Apply", 1);
    for (id, node) in &mut reordered.nodes {
        if *id == NodeId(3) {
            node.set_children(vec![NodeId(6), NodeId(4)]);
        }
    }
    let after = project(&reordered, 1.0);
    assert_eq!(
        diff(&before, &after),
        [
            DomOp::Place {
                id: "stobie-a11y:status".into(),
                parent: Some("stobie-a11y:pane.2".into()),
                before: Some("stobie-a11y:record-desk.apply".into()),
            },
            DomOp::Remove {
                id: "stobie-a11y:egui-5".into(),
            },
        ]
    );
}

#[test]
fn a_new_first_child_of_a_long_list_is_the_only_placement() {
    let list = |first: bool| {
        let mut nodes = vec![];
        let mut children: Vec<u64> = (10..90).collect();
        if first {
            children.insert(0, 9);
        }
        nodes.push((1, node(Role::Window, &children)));
        for id in children {
            let mut item = node(Role::Button, &[]);
            item.set_label(format!("Item {id}"));
            nodes.push((id, item));
        }
        project(&update(nodes, 1), 1.0)
    };
    let operations = diff(&list(false), &list(true));
    let places: Vec<_> = operations
        .iter()
        .filter(|operation| matches!(operation, DomOp::Place { .. }))
        .collect();
    assert_eq!(
        places,
        [&DomOp::Place {
            id: "stobie-a11y:egui-9".into(),
            parent: None,
            before: Some("stobie-a11y:egui-a".into()),
        }]
    );
    assert_eq!(operations.len(), 5, "create, role, label, style and place");
}

#[test]
fn moving_an_ancestor_below_its_former_child_places_top_down() {
    // Before: A contains B. After: B contains A.
    let mut a = node(Role::Group, &[3]);
    a.set_label("A");
    let mut b = node(Role::Group, &[]);
    b.set_label("B");
    let before = project(
        &update(
            vec![
                (1, node(Role::Window, &[2])),
                (2, a.clone()),
                (3, b.clone()),
            ],
            1,
        ),
        1.0,
    );
    a.set_children(Vec::<NodeId>::new());
    b.set_children(vec![NodeId(2)]);
    let after = project(
        &update(vec![(1, node(Role::Window, &[3])), (2, a), (3, b)], 1),
        1.0,
    );
    let places: Vec<_> = diff(&before, &after)
        .into_iter()
        .filter(|operation| matches!(operation, DomOp::Place { .. }))
        .collect();
    assert_eq!(
        places,
        [
            DomOp::Place {
                id: "stobie-a11y:egui-3".into(),
                parent: None,
                before: None,
            },
            DomOp::Place {
                id: "stobie-a11y:egui-2".into(),
                parent: Some("stobie-a11y:egui-3".into()),
                before: None,
            },
        ]
    );
}

#[test]
fn a_changed_tag_recreates_the_element_in_place() {
    let field = |multiline: bool| {
        let mut field = node(
            if multiline {
                Role::MultilineTextInput
            } else {
                Role::TextInput
            },
            &[],
        );
        field.set_label("Notes");
        field.set_value("Draft");
        let mut first = node(Role::Button, &[]);
        first.set_label("First");
        let mut last = node(Role::Button, &[]);
        last.set_label("Last");
        project(
            &update(
                vec![
                    (1, node(Role::Window, &[2, 3, 4])),
                    (2, first),
                    (3, field),
                    (4, last),
                ],
                1,
            ),
            1.0,
        )
    };
    let single = field(false);
    let multiline = field(true);
    assert_eq!(single.elements["stobie-a11y:egui-3"].tag, "input");
    assert_eq!(multiline.elements["stobie-a11y:egui-3"].tag, "textarea");
    let operations = diff(&single, &multiline);
    assert_eq!(
        operations[..2],
        [
            DomOp::Remove {
                id: "stobie-a11y:egui-3".into()
            },
            DomOp::Create {
                id: "stobie-a11y:egui-3".into(),
                tag: "textarea",
            },
        ]
    );
    assert!(operations.contains(&DomOp::SetText {
        id: "stobie-a11y:egui-3".into(),
        text: Some("Draft".into()),
    }));
    assert!(operations.contains(&DomOp::Place {
        id: "stobie-a11y:egui-3".into(),
        parent: None,
        before: Some("stobie-a11y:egui-4".into()),
    }));
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation, DomOp::Place { .. }))
            .count(),
        1,
        "only the recreated element is placed"
    );
}

#[test]
fn longest_increasing_keeps_the_most_children_in_place() {
    let stable = longest_increasing(&[Some(3), None, Some(0), Some(1), Some(5), Some(2)]);
    assert_eq!(stable.into_iter().collect::<Vec<_>>(), [2, 3, 5]);
    assert!(longest_increasing(&[None, None]).is_empty());
}

#[test]
fn numbers_are_formatted_without_float_noise() {
    assert_eq!(number(0.30000000000000004), "0.3");
    assert_eq!(number(2.0), "2");
    assert_eq!(number(-0.00001), "0");
    assert_eq!(number(12.5), "12.5");
}
