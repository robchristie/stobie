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

#[test]
fn enabled_clickable_elements_are_hit_targets_and_carry_semantic_ids() {
    let tree = project(&sample("Apply", 1), 1.0);
    let button = &tree.elements["stobie-a11y:record-desk.apply"];
    assert_eq!(button.attributes[HIT_ATTRIBUTE], "true");
    assert_eq!(
        button.attributes[SEMANTIC_ID_ATTRIBUTE],
        "record-desk.apply"
    );
    // A region without a click action, and text without an author, are not.
    let pane = &tree.elements["stobie-a11y:pane.2"];
    assert!(!pane.attributes.contains_key(HIT_ATTRIBUTE));
    assert_eq!(pane.attributes[SEMANTIC_ID_ATTRIBUTE], "pane.2");
    assert!(
        tree.elements
            .values()
            .filter(|element| element.text.as_deref() == Some("Committed record"))
            .all(|element| element.attributes.is_empty())
    );

    // A disabled control, a read-only text field and an empty box take no
    // pointer.
    let mut disabled = node(Role::Button, &[]);
    disabled.set_label("Undo");
    disabled.add_action(Action::Click);
    disabled.set_disabled();
    bounds(&mut disabled, 0.0, 0.0, 40.0, 20.0);
    let mut field = node(Role::TextInput, &[]);
    field.set_label("Title");
    field.add_action(Action::Click);
    field.set_read_only();
    bounds(&mut field, 0.0, 30.0, 40.0, 50.0);
    let mut empty = node(Role::Button, &[]);
    empty.set_label("Hidden");
    empty.add_action(Action::Click);
    bounds(&mut empty, 0.0, 60.0, 0.0, 60.0);
    let tree = project(
        &update(
            vec![
                (1, node(Role::Window, &[2, 3, 4])),
                (2, disabled),
                (3, field),
                (4, empty),
            ],
            1,
        ),
        1.0,
    );
    assert!(
        tree.elements
            .values()
            .all(|element| !element.attributes.contains_key(HIT_ATTRIBUTE))
    );

    // Enabling the control writes only its hit marker.
    let enabled = sample("Apply", 1);
    let mut disabled = sample("Apply", 1);
    for (_, node) in &mut disabled.nodes {
        if node.role() == Role::Button {
            node.set_disabled();
        }
    }
    let operations = diff(&project(&disabled, 1.0), &project(&enabled, 1.0));
    assert!(operations.contains(&DomOp::SetAttribute {
        id: "stobie-a11y:record-desk.apply".into(),
        name: HIT_ATTRIBUTE,
        value: "true".into(),
    }));
}

#[test]
fn only_controls_visible_within_their_clipping_ancestors_are_hit_targets() {
    let option = |label: &str, y0: f64| {
        let mut option = node(Role::ListBoxOption, &[]);
        option.set_label(label);
        option.add_action(Action::Click);
        bounds(&mut option, 0.0, y0, 200.0, y0 + 20.0);
        option
    };
    // A scroll area (flattened) shows 0..100; its list box over-renders rows.
    let mut scroll = node(Role::ScrollView, &[3]);
    bounds(&mut scroll, 0.0, 0.0, 200.0, 100.0);
    let mut list = node(Role::ListBox, &[4, 5, 6]);
    list.set_label("Results");
    bounds(&mut list, 0.0, 0.0, 200.0, 300.0);
    let tree = project(
        &update(
            vec![
                (1, node(Role::Window, &[2])),
                (2, scroll),
                (3, list),
                (4, option("Visible", 10.0)),
                (5, option("Half shown", 85.0)),
                (6, option("Clipped", 140.0)),
            ],
            1,
        ),
        1.0,
    );
    let hit = |label: &str| {
        tree.elements
            .values()
            .find(|element| element.attributes.get("aria-label").map(String::as_str) == Some(label))
            .expect("projected")
            .attributes
            .contains_key(HIT_ATTRIBUTE)
    };
    assert!(hit("Visible"));
    // Its centre (y 95) is still shown.
    assert!(hit("Half shown"));
    // Drawn nowhere, so a pixel at its centre would cover another control.
    assert!(!hit("Clipped"));

    // Nothing beyond the window is shown either.
    let mut window = node(Role::Window, &[2]);
    bounds(&mut window, 0.0, 0.0, 400.0, 300.0);
    let tree = project(
        &update(vec![(1, window), (2, option("Below the window", 310.0))], 1),
        1.0,
    );
    assert!(
        tree.elements
            .values()
            .all(|element| !element.attributes.contains_key(HIT_ATTRIBUTE))
    );
}

#[test]
fn a_control_whose_centre_lies_on_another_control_takes_no_pointer() {
    let mut row = node(Role::ListBoxOption, &[]);
    row.set_label("Edge row");
    row.add_action(Action::Click);
    bounds(&mut row, 0.0, 80.0, 200.0, 100.0);
    // A splitter's grab area reaches over the row's centre.
    let mut splitter = node(Role::Splitter, &[]);
    splitter.set_label("Horizontal splitter");
    splitter.add_action(Action::Focus);
    bounds(&mut splitter, 0.0, 88.0, 400.0, 96.0);
    let mut list = node(Role::ListBox, &[3]);
    list.set_label("Results");
    bounds(&mut list, 0.0, 0.0, 200.0, 100.0);
    let mut button = node(Role::Button, &[]);
    button.set_label("Apply");
    button.add_action(Action::Click);
    bounds(&mut button, 300.0, 0.0, 360.0, 20.0);
    let tree = project(
        &update(
            vec![
                (1, node(Role::Window, &[2, 4, 5])),
                (2, list),
                (3, row),
                (4, splitter),
                (5, button),
            ],
            1,
        ),
        1.0,
    );
    let hit = |label: &str| {
        tree.elements
            .values()
            .find(|element| element.attributes.get("aria-label").map(String::as_str) == Some(label))
            .expect("projected")
            .attributes
            .contains_key(HIT_ATTRIBUTE)
    };
    // The splitter comes later, so egui would give it the click.
    assert!(!hit("Edge row"));
    assert!(hit("Apply"));

    // A splitter that comes first, as a dock registers it before its panes,
    // lies beneath the row, which keeps its target.
    let mut row = node(Role::ListBoxOption, &[]);
    row.set_label("Edge row");
    row.add_action(Action::Click);
    bounds(&mut row, 0.0, 80.0, 200.0, 100.0);
    let mut splitter = node(Role::Splitter, &[]);
    splitter.set_label("Horizontal splitter");
    bounds(&mut splitter, 0.0, 88.0, 400.0, 96.0);
    let mut list = node(Role::ListBox, &[4]);
    list.set_label("Results");
    bounds(&mut list, 0.0, 0.0, 200.0, 100.0);
    let tree = project(
        &update(
            vec![
                (1, node(Role::Window, &[2, 3])),
                (2, splitter),
                (3, list),
                (4, row),
            ],
            1,
        ),
        1.0,
    );
    assert!(
        tree.elements
            .values()
            .find(
                |element| element.attributes.get("aria-label").map(String::as_str)
                    == Some("Edge row")
            )
            .unwrap()
            .attributes
            .contains_key(HIT_ATTRIBUTE)
    );
}

#[test]
fn a_row_clipped_out_of_its_list_does_not_contest_a_visible_control_beneath() {
    // A tab strip at y 0..30, then a list from y 40 whose scrolled rows are
    // still reported above it, over the tab.
    let mut tab = node(Role::Tab, &[]);
    tab.set_label("Thumbnails");
    tab.add_action(Action::Click);
    bounds(&mut tab, 0.0, 0.0, 100.0, 30.0);
    let mut row = node(Role::ListBoxOption, &[]);
    row.set_label("Scrolled away");
    row.add_action(Action::Click);
    bounds(&mut row, 0.0, 5.0, 200.0, 25.0);
    let mut list = node(Role::ListBox, &[4]);
    list.set_label("Results");
    bounds(&mut list, 0.0, 40.0, 200.0, 300.0);
    let tree = project(
        &update(
            vec![
                (1, node(Role::Window, &[2, 3])),
                (2, tab),
                (3, list),
                (4, row),
            ],
            1,
        ),
        1.0,
    );
    let hit = |label: &str| {
        tree.elements
            .values()
            .find(|element| element.attributes.get("aria-label").map(String::as_str) == Some(label))
            .expect("projected")
            .attributes
            .contains_key(HIT_ATTRIBUTE)
    };
    assert!(hit("Thumbnails"));
    assert!(!hit("Scrolled away"));
}

#[test]
fn an_editable_text_field_is_its_own_centre_pixel_hit_target() {
    let mut field = node(Role::MultilineTextInput, &[]);
    field.set_label("Record notes");
    field.set_author_id("record-desk.notes.1001".to_owned());
    bounds(&mut field, 0.0, 30.0, 40.0, 50.0);
    let tree = project(
        &update(vec![(1, node(Role::Window, &[2])), (2, field)], 1),
        1.0,
    );
    let field = &tree.elements["stobie-a11y:record-desk.notes.1001"];
    assert_eq!(field.tag, "textarea");
    assert_eq!(field.attributes[HIT_ATTRIBUTE], "true");
    // Only the centre pixel takes pointer input; the rest stays the canvas's.
    assert!(
        field
            .style
            .ends_with(";clip-path:inset(calc(50% - 0.5px));touch-action:none"),
        "{}",
        field.style
    );

    // A later control over its centre takes the pointer there instead.
    let mut field = node(Role::TextInput, &[]);
    field.set_label("Title");
    bounds(&mut field, 0.0, 0.0, 40.0, 20.0);
    let mut button = node(Role::Button, &[]);
    button.set_label("Clear");
    button.add_action(Action::Click);
    bounds(&mut button, 10.0, 5.0, 30.0, 15.0);
    let tree = project(
        &update(
            vec![(1, node(Role::Window, &[2, 3])), (2, field), (3, button)],
            1,
        ),
        1.0,
    );
    let field = tree
        .elements
        .values()
        .find(|element| element.tag == "input")
        .unwrap();
    assert!(!field.attributes.contains_key(HIT_ATTRIBUTE));
}

/// A window with a pane holding a title field, a read-only field, a disabled
/// field, notes clipped by a scroll area and a button over part of the notes.
fn fields() -> TreeUpdate {
    let field = |role: Role, label: &str, y0: f64| {
        let mut field = node(role, &[]);
        field.set_label(label);
        bounds(&mut field, 0.0, y0, 200.0, y0 + 20.0);
        field
    };
    let mut read_only = field(Role::TextInput, "Read only", 30.0);
    read_only.set_read_only();
    let mut disabled = field(Role::TextInput, "Disabled", 60.0);
    disabled.set_disabled();
    let mut notes = field(Role::MultilineTextInput, "Notes", 90.0);
    bounds(&mut notes, 0.0, 90.0, 200.0, 150.0);
    let mut scroll = node(Role::ScrollView, &[6]);
    bounds(&mut scroll, 0.0, 90.0, 200.0, 120.0);
    let mut button = node(Role::Button, &[]);
    button.set_label("Send");
    button.add_action(Action::Click);
    bounds(&mut button, 150.0, 90.0, 200.0, 110.0);
    update(
        vec![
            (1, node(Role::Window, &[2, 3, 4, 5, 7])),
            (2, field(Role::TextInput, "Title", 0.0)),
            (3, read_only),
            (4, disabled),
            (5, scroll),
            (6, notes),
            (7, button),
        ],
        1,
    )
}

fn label_of<'a>(tree: &'a AriaTree, id: Option<&str>) -> Option<&'a str> {
    id.and_then(|id| tree.elements[id].attributes.get("aria-label"))
        .map(String::as_str)
}

#[test]
fn a_tap_finds_the_topmost_editable_text_field_shown_there() {
    let tree = project(&fields(), 1.0);
    let at = |x, y| label_of(&tree, text_field_at(&tree, (x, y)));
    assert_eq!(at(100.0, 10.0), Some("Title"));
    // Read-only and disabled fields take no typing.
    assert_eq!(at(100.0, 40.0), None);
    assert_eq!(at(100.0, 70.0), None);
    // The scroll area shows the notes down to 120 only.
    assert_eq!(at(100.0, 100.0), Some("Notes"));
    assert_eq!(at(100.0, 130.0), None);
    // A later control over the field takes the tap.
    assert_eq!(at(160.0, 100.0), None);
    // Nothing is there.
    assert_eq!(at(300.0, 10.0), None);
}

/// The AccessKit update of a pass that shows a plain egui field labelled
/// only by a visible label, as `Response::labelled_by` does, and focuses it.
fn labelled_field_update() -> TreeUpdate {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut title = String::from("Compare field notes");
    let mut update = None;
    for _ in 0..2 {
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            let label = ui.label("Title");
            let field = ui.add(egui::TextEdit::singleline(&mut title));
            let field = field.labelled_by(label.id);
            ui.memory_mut(|memory| memory.request_focus(field.id));
        });
        output.textures_delta.clear();
        update = output.platform_output.accesskit_update;
    }
    update.expect("an AccessKit update")
}

#[test]
fn a_field_labelled_by_a_visible_label_is_named_by_it() {
    let tree = project(&labelled_field_update(), 1.0);
    let field = tree.active.as_deref().expect("the focused field");
    assert_eq!(tree.elements[field].tag, "input");
    // egui names the field only through the relationship, so a name taken
    // from `aria-label` alone would be missing.
    assert!(!tree.elements[field].attributes.contains_key("aria-label"));
    assert!(
        tree.elements[field]
            .attributes
            .contains_key("aria-labelledby")
    );
    assert_eq!(tree.accessible_name(field).as_deref(), Some("Title"));

    // The relationship takes precedence over `aria-label`, as in browsers,
    // and `aria-label` names a field without one.
    let mut caption = node(Role::Label, &[]);
    caption.set_value("Record title");
    let mut labelled = node(Role::TextInput, &[]);
    labelled.set_label("Title field");
    labelled.push_labelled_by(NodeId(2));
    labelled.set_author_id("labelled");
    let mut plain = node(Role::TextInput, &[]);
    plain.set_label("Record notes");
    plain.set_author_id("plain");
    let mut unnamed = node(Role::TextInput, &[]);
    unnamed.set_author_id("unnamed");
    let tree = project(
        &update(
            vec![
                (1, node(Role::Window, &[2, 3, 4, 5])),
                (2, caption),
                (3, labelled),
                (4, plain),
                (5, unnamed),
            ],
            1,
        ),
        1.0,
    );
    let name = |id: &str| tree.accessible_name(&format!("stobie-a11y:{id}"));
    assert_eq!(name("labelled").as_deref(), Some("Record title"));
    assert_eq!(name("plain").as_deref(), Some("Record notes"));
    assert_eq!(name("unnamed"), None);
}

#[test]
fn a_tap_stays_within_egui_click_distance_and_time() {
    assert!(is_tap((10.0, 10.0), (14.0, 14.0), 0.1));
    assert!(!is_tap((10.0, 10.0), (20.0, 10.0), 0.1), "a drag scrolls");
    assert!(!is_tap((10.0, 10.0), (10.0, 10.0), 0.9), "a long press");
}

fn empty_galley() -> std::sync::Arc<egui::Galley> {
    let context = egui::Context::default();
    let mut galley = None;
    let mut output = context.run_ui(Default::default(), |ui| {
        galley = Some(ui.painter().layout_no_wrap(
            String::new(),
            egui::FontId::default(),
            egui::Color32::WHITE,
        ));
    });
    output.textures_delta.clear();
    galley.expect("laid out")
}

fn registration(id: &str) -> NativeTextRegistration {
    let rect = egui::Rect::from_min_max(egui::pos2(10.0, 200.0), egui::pos2(210.0, 240.0));
    NativeTextRegistration {
        id: egui::Id::new(id),
        multiline: true,
        rect,
        text_rect: rect.shrink2(egui::vec2(4.0, 2.0)),
        clip: egui::Rect::EVERYTHING,
        galley: empty_galley(),
        galley_pos: rect.min,
        family: NativeFont::Proportional,
        font_size: 17.0,
        line_height: 21.0,
        text: egui::Color32::from_rgb(240, 240, 240),
        caret: egui::Color32::from_rgb(90, 160, 255),
        selection: egui::Color32::from_rgba_unmultiplied(90, 160, 255, 128),
        placeholder: egui::Color32::from_rgb(140, 140, 140),
        hint: "Message".to_owned(),
        spellcheck: true,
    }
}

fn composer_update(id: &str) -> TreeUpdate {
    let mut field = node(Role::MultilineTextInput, &[]);
    field.set_label("Message");
    field.set_author_id("chat.draft");
    bounds(&mut field, 10.0, 200.0, 210.0, 240.0);
    let mut other = node(Role::TextInput, &[]);
    other.set_label("Search");
    bounds(&mut other, 10.0, 10.0, 210.0, 30.0);
    update(
        vec![
            (1, node(Role::Window, &[egui::Id::new(id).value(), 3])),
            (egui::Id::new(id).value(), field),
            (3, other),
        ],
        1,
    )
}

#[test]
fn a_registered_field_is_marked_native_with_its_active_presentation() {
    let tree = project_native(
        &composer_update("draft"),
        1.0,
        None,
        &[registration("draft")],
    );
    let field = &tree.elements["stobie-a11y:chat.draft"];
    assert_eq!(field.tag, "textarea");
    assert_eq!(field.attributes[NATIVE_ATTRIBUTE], "true");
    assert_eq!(field.attributes["spellcheck"], "true");
    assert_eq!(field.attributes["autocorrect"], "on");
    assert_eq!(field.attributes["autocapitalize"], "sentences");
    // Other fields are unchanged.
    let search = tree
        .elements
        .values()
        .find(|element| element.tag == "input")
        .unwrap();
    assert!(!search.attributes.contains_key(NATIVE_ATTRIBUTE));
    assert_eq!(search.attributes["spellcheck"], "false");
    assert_eq!(tree.native.len(), 1);

    let native = &tree.native["stobie-a11y:chat.draft"];
    for part in [
        "left:10.0px;top:200.0px;width:200.0px;height:40.0px;padding:2.0px 4.0px 2.0px 4.0px;",
        "font-family:\"Stobie Source Sans 3 Regular\",system-ui,-apple-system,sans-serif;font-size:17.0px;",
        "line-height:21.0px;color:rgba(240,240,240,1);",
        "caret-color:rgba(90,160,255,1);",
        "background:transparent;",
        "pointer-events:auto;user-select:text;",
    ] {
        assert!(native.style.contains(part), "{part} in {}", native.style);
    }
    assert!(!native.style.contains("clip-path"), "{}", native.style);
    assert!(!native.style.contains("transform"), "{}", native.style);
    assert_eq!(
        native.rules,
        "[data-stobie-native-active]::selection{background-color:rgba(90,159,255,0.502)}[data-stobie-native-active]::placeholder{color:rgba(140,140,140,1);-webkit-text-fill-color:rgba(140,140,140,1);opacity:1}"
    );
    assert_eq!(native.placeholder, "Message");

    // A field turned out of native mode loses its marker on the next diff.
    let plain = project(&composer_update("draft"), 1.0);
    assert!(diff(&tree, &plain).contains(&DomOp::RemoveAttribute {
        id: "stobie-a11y:chat.draft".to_owned(),
        name: NATIVE_ATTRIBUTE,
    }));
    assert!(plain.native.is_empty());
}

#[test]
fn a_native_field_scales_clips_and_never_zooms_the_page() {
    // At zoom 2, 17-point text is 34 CSS pixels.
    let tree = project_native(
        &composer_update("draft"),
        2.0,
        None,
        &[registration("draft")],
    );
    let native = &tree.native["stobie-a11y:chat.draft"];
    assert!(
        native
            .style
            .contains("left:20.0px;top:400.0px;width:400.0px;height:80.0px;"),
        "{}",
        native.style
    );
    assert!(native.style.contains("font-size:34.0px;"));

    // Text under 16 CSS pixels is laid out at 16 and scaled down to fit.
    let mut small = registration("draft");
    small.font_size = 12.0;
    small.line_height = 15.0;
    small.spellcheck = false;
    let tree = project_native(&composer_update("draft"), 1.0, None, &[small.clone()]);
    let native = &tree.native["stobie-a11y:chat.draft"];
    for part in [
        "left:10.0px;top:200.0px;width:266.7px;height:53.3px;padding:2.7px 5.3px 2.7px 5.3px;",
        "font-size:16.0px;",
        "line-height:20.0px;",
        "transform:scale(0.75);transform-origin:0 0;",
    ] {
        assert!(native.style.contains(part), "{part} in {}", native.style);
    }
    assert_eq!(
        tree.elements["stobie-a11y:chat.draft"].attributes["autocorrect"],
        "off"
    );

    // Only what the field's scroll area shows of it is drawn.
    small.font_size = 17.0;
    small.clip = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 230.0));
    let tree = project_native(&composer_update("draft"), 1.0, None, &[small]);
    let native = &tree.native["stobie-a11y:chat.draft"];
    assert!(
        native
            .style
            .contains("clip-path:inset(0.0px 0.0px 10.0px 0.0px);"),
        "{}",
        native.style
    );
}

#[test]
fn a_tapped_field_awaiting_egui_ime_output_gets_its_rectangle_and_purpose() {
    let mut name = node(Role::TextInput, &[]);
    bounds(&mut name, 10.0, 20.0, 110.0, 44.0);
    let mut password = node(Role::PasswordInput, &[]);
    bounds(&mut password, 10.0, 60.0, 110.0, 84.0);
    let nodes = || {
        vec![
            (1, node(Role::Window, &[2, 3])),
            (2, name.clone()),
            (3, password.clone()),
        ]
    };
    let ime = tapped_field_ime(&update(nodes(), 2), NodeId(2)).unwrap();
    assert_eq!(ime.purpose, egui::IMEPurpose::Normal);
    assert_eq!(
        ime.rect,
        egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(110.0, 44.0))
    );
    assert_eq!(ime.cursor_rect, ime.rect);
    // A password field's typed text must be handled as a password's, or
    // eframe repeats what was typed before egui's own output arrives.
    let ime = tapped_field_ime(&update(nodes(), 3), NodeId(3)).unwrap();
    assert_eq!(ime.purpose, egui::IMEPurpose::Password);
    // Only while egui's focus is on the tapped field.
    assert_eq!(tapped_field_ime(&update(nodes(), 2), NodeId(3)), None);
}
