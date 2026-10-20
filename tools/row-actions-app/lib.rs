//! The per-row actions check's application. `tools/outside-app-smoke.sh`
//! creates `outside-rows` with `stobie new`, replaces its `src/lib.rs` with
//! this file, and `tools/row-actions-smoke.mjs` operates it natively and in
//! Chromium through `stobie` and the browser's accessibility tree.
//!
//! A list of people presents each of its actions once per row, carrying the
//! row's ID: Open, Remove (an icon) and Pinned (a toggle) on each row, and the
//! row itself, which selects the person. Agents discover each row's control
//! with the ID it carries and invoke it by that ID.

use serde::Serialize;
use stobie_ui_egui::{
    Action, ActionArgument, ActionEmphasis, ActionKey, ActionTarget, Application,
    ApplicationIdentity, Availability, BoundAction, Frame, IconId, ListRowSpec, Observation,
    PresentationScope, SemanticUiId, application_identity,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "outside-rows")]
pub enum RowAction {
    #[action(
        label = "Open",
        description = "Open the person with this ID",
        argument = "number",
        minimum = 1,
        maximum = 999,
        integer = true
    )]
    Open,
    #[action(
        label = "Remove",
        description = "Remove the person with this ID from the list",
        argument = "number",
        minimum = 1,
        maximum = 999,
        integer = true
    )]
    Remove,
    #[action(
        label = "Pinned",
        description = "Keep the person with this ID at the top of the list",
        argument = "number",
        minimum = 1,
        maximum = 999,
        integer = true
    )]
    Pin,
    #[action(
        label = "Select",
        description = "Select the person with this ID, such as ada",
        argument = "text",
        max_length = 16
    )]
    Select,
    #[action(label = "Restore", description = "Bring back everyone removed")]
    Restore,
}

pub enum RowIntent {}

struct Person {
    id: u32,
    key: &'static str,
    name: &'static str,
    role: &'static str,
    pinned: bool,
}

const PEOPLE: [(u32, &str, &str, &str); 3] = [
    (1, "ada", "Ada Lovelace", "Mathematician"),
    (
        2,
        "grace",
        "Grace Hopper",
        "Rear admiral and computer scientist",
    ),
    (3, "edsger", "Edsger Dijkstra", "Computer scientist"),
];

pub struct App {
    people: Vec<Person>,
    opened: String,
    selected: String,
    error: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            people: everyone(),
            opened: String::new(),
            selected: String::new(),
            error: String::new(),
        }
    }
}

fn everyone() -> Vec<Person> {
    PEOPLE
        .iter()
        .map(|&(id, key, name, role)| Person {
            id,
            key,
            name,
            role,
            pinned: false,
        })
        .collect()
}

impl App {
    /// The person an argument names: an ID number or a text key.
    fn person(&self, argument: &ActionArgument) -> Option<&Person> {
        self.people.iter().find(|person| match argument {
            ActionArgument::Number(id) => f64::from(person.id) == *id,
            ActionArgument::Text(key) => person.key == key,
            _ => false,
        })
    }

    /// Pinned people first, each group in ID order.
    fn ordered(&self) -> Vec<&Person> {
        let mut people: Vec<_> = self.people.iter().collect();
        people.sort_by_key(|person| (!person.pinned, person.id));
        people
    }
}

impl Application for App {
    type Action = RowAction;
    type Intent = RowIntent;
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        application_identity!("Outside Rows")
    }

    fn availability(&self, target: ActionTarget<RowAction>) -> Availability {
        match target.action {
            RowAction::Restore if self.people.len() == PEOPLE.len() => Availability::Disabled {
                reason: "Nobody has been removed".into(),
            },
            _ => Availability::Enabled,
        }
    }

    /// A row's action is available while its person is listed; a pinned
    /// person cannot be removed.
    fn availability_with(
        &self,
        target: ActionTarget<RowAction>,
        argument: &ActionArgument,
    ) -> Availability {
        match self.person(argument) {
            None => Availability::Hidden,
            Some(person) if target.action == RowAction::Remove && person.pinned => {
                Availability::Disabled {
                    reason: format!("Unpin {} first", person.name).into(),
                }
            }
            Some(_) => Availability::Enabled,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, _: &mut (), frame: &mut Frame<'_, RowAction, RowIntent>) {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut p =
                frame.presentation(ui, PresentationScope::new("people"), SemanticUiId::root());
            p.heading(ui, "title", "People");
            for person in self.ordered() {
                // The row selects its person; its buttons act on them.
                frame.list_row(
                    ui,
                    &mut p,
                    BoundAction::new(RowAction::Select, person.key),
                    ListRowSpec {
                        title: person.name,
                        detail: Some(person.role),
                    },
                );
                ui.horizontal(|ui| {
                    let subject =
                        |action| BoundAction::new(action, person.id).with_subject(person.name);
                    frame.action(ui, &mut p, subject(RowAction::Open), ActionEmphasis::Quiet);
                    frame.toggle(ui, &mut p, "pinned", subject(RowAction::Pin), person.pinned);
                    frame.icon_action(
                        ui,
                        &mut p,
                        subject(RowAction::Remove),
                        IconId::Remove,
                        ActionEmphasis::Quiet,
                    );
                });
                ui.separator();
            }
            frame.action(ui, &mut p, RowAction::Restore, ActionEmphasis::Normal);
            let status = if !self.error.is_empty() {
                self.error.clone()
            } else if let Some(person) = PEOPLE.iter().find(|person| person.1 == self.opened) {
                format!("Opened {} · {} people", person.2, self.people.len())
            } else {
                format!("{} people", self.people.len())
            };
            p.status(ui, "status", &status, !self.error.is_empty());
        });
    }

    fn apply(&mut self, intent: RowIntent, _: &mut ()) -> Result<bool, String> {
        match intent {}
    }

    fn perform(&mut self, target: ActionTarget<RowAction>, _: &mut ()) -> Result<bool, String> {
        match target.action {
            RowAction::Restore => {
                let pinned: Vec<u32> = self
                    .people
                    .iter()
                    .filter(|person| person.pinned)
                    .map(|person| person.id)
                    .collect();
                self.people = everyone();
                for person in &mut self.people {
                    person.pinned = pinned.contains(&person.id);
                }
                self.error.clear();
                Ok(true)
            }
            action => Err(format!("{} needs a person", action.specification().label)),
        }
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<RowAction>,
        argument: ActionArgument,
        state: &mut (),
    ) -> Result<bool, String> {
        let Some(id) = self.person(&argument).map(|person| person.id) else {
            return self.perform(target, state);
        };
        let person = self
            .people
            .iter_mut()
            .find(|person| person.id == id)
            .expect("an available person");
        match target.action {
            RowAction::Open => self.opened = person.key.into(),
            RowAction::Pin => person.pinned = !person.pinned,
            RowAction::Select => self.selected = person.key.into(),
            RowAction::Remove => {
                self.people.retain(|person| person.id != id);
                if self.selected == PEOPLE[id as usize - 1].1 {
                    self.selected.clear();
                }
            }
            RowAction::Restore => return self.perform(target, state),
        }
        self.error.clear();
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        self.error = message;
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        observation.fact("people", self.people.len());
        observation.fact("opened", self.opened.as_str());
        observation.fact("selected", self.selected.as_str());
        let pinned: Vec<_> = self
            .people
            .iter()
            .filter(|person| person.pinned)
            .map(|person| person.key)
            .collect();
        observation.fact("pinned", pinned.join(","));
        observation.fact("error", self.error.as_str());
    }
}

#[cfg(target_arch = "wasm32")]
stobie_ui_egui::web_handle!(App, |_| App::default());
