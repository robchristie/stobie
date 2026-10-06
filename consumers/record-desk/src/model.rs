//! Record Desk's domain state. Drafts and history stay outside saved state.

use std::collections::{BTreeSet, VecDeque};

use polyorama_core::{DockNode, DockNodeId, LAYOUT_SCHEMA_VERSION, PaneId, SplitAxis, Workspace};
use serde::{Deserialize, Serialize};

pub const LIST_PANE: PaneId = PaneId(101);
pub const DETAIL_PANE: PaneId = PaneId(102);
pub const VERSION: u32 = 1;
pub const MAX_STATE_BYTES: usize = 1024 * 1024;
const MAX_RECORDS: usize = 1000;
const MAX_HISTORY: usize = 100;
const MAX_LAYOUT_NODES: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecordId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Research,
    Operations,
    Ideas,
}

impl Category {
    pub const ALL: [Self; 3] = [Self::Research, Self::Operations, Self::Ideas];

    pub fn label(self) -> &'static str {
        match self {
            Self::Research => "Research",
            Self::Operations => "Operations",
            Self::Ideas => "Ideas",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewFilter {
    #[default]
    All,
    Reviewed,
    Unreviewed,
}

impl ReviewFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Reviewed, Self::Unreviewed];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All records",
            Self::Reviewed => "Reviewed",
            Self::Unreviewed => "Unreviewed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub id: RecordId,
    pub title: String,
    pub category: Category,
    pub notes: String,
    pub reviewed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filters {
    pub query: String,
    pub category: Option<Category>,
    pub review: ReviewFilter,
}

#[derive(Clone, Debug)]
struct Edit {
    before: Record,
    after: Record,
}

#[derive(Clone, Debug)]
pub struct Desk {
    records: Vec<Record>,
    selected: Option<RecordId>,
    draft: Option<Record>,
    filters: Filters,
    undo: VecDeque<Edit>,
    redo: Vec<Edit>,
    draft_revision: u64,
    history_revision: u64,
    filter_revision: u64,
}

impl Desk {
    pub fn new(records: Vec<Record>) -> Result<Self, String> {
        validate_records(&records)?;
        let draft = records.first().cloned();
        Ok(Self {
            selected: draft.as_ref().map(|record| record.id),
            draft,
            records,
            filters: Filters::default(),
            undo: VecDeque::new(),
            redo: Vec::new(),
            draft_revision: 0,
            history_revision: 0,
            filter_revision: 0,
        })
    }

    pub fn records(&self) -> &[Record] {
        &self.records
    }

    pub fn selected(&self) -> Option<RecordId> {
        self.selected
    }

    pub fn draft(&self) -> Option<&Record> {
        self.draft.as_ref()
    }

    pub fn filters(&self) -> &Filters {
        &self.filters
    }

    pub fn draft_revision(&self) -> u64 {
        self.draft_revision
    }
    pub fn history_revision(&self) -> u64 {
        self.history_revision
    }
    pub fn filter_revision(&self) -> u64 {
        self.filter_revision
    }

    pub fn visible(&self) -> Vec<&Record> {
        let query = self.filters.query.trim().to_lowercase();
        self.records
            .iter()
            .filter(|record| {
                self.filters
                    .category
                    .is_none_or(|category| record.category == category)
                    && match self.filters.review {
                        ReviewFilter::All => true,
                        ReviewFilter::Reviewed => record.reviewed,
                        ReviewFilter::Unreviewed => !record.reviewed,
                    }
                    && (query.is_empty()
                        || record.title.to_lowercase().contains(&query)
                        || record.notes.to_lowercase().contains(&query))
            })
            .collect()
    }

    pub fn is_dirty(&self) -> bool {
        self.draft.as_ref() != self.selected_record()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn set_filters(&mut self, filters: Filters) {
        if self.filters != filters {
            self.filter_revision += 1;
        }
        self.filters = filters;
    }

    pub fn select(&mut self, id: RecordId) -> Result<(), String> {
        if self.selected == Some(id) {
            return Ok(());
        }
        if self.is_dirty() {
            return Err("Apply or cancel the draft before selecting another record.".into());
        }
        let record = self
            .records
            .iter()
            .find(|record| record.id == id)
            .ok_or_else(|| "The selected record does not exist.".to_owned())?;
        self.selected = Some(id);
        self.draft = Some(record.clone());
        self.draft_revision += 1;
        Ok(())
    }

    /// Accept an editable draft, including temporarily invalid fields. Apply is
    /// the validation boundary; changing a record's identity is never allowed.
    pub fn edit_draft(&mut self, record: Record) -> Result<(), String> {
        if self.selected != Some(record.id) {
            return Err("The draft must retain the selected record ID.".into());
        }
        if self.draft.as_ref() != Some(&record) {
            self.draft_revision += 1;
        }
        self.draft = Some(record);
        Ok(())
    }

    pub fn apply(&mut self) -> Result<bool, String> {
        let mut after = self
            .draft
            .clone()
            .ok_or_else(|| "Select a record before applying a draft.".to_owned())?;
        validate_record(&after)?;
        after.title = after.title.trim().to_owned();
        after.notes = after.notes.trim().to_owned();
        let index = self
            .records
            .iter()
            .position(|record| Some(record.id) == self.selected)
            .ok_or_else(|| "The selected record does not exist.".to_owned())?;
        if self.records[index] == after {
            if self.draft.as_ref() != Some(&after) {
                self.draft_revision += 1;
            }
            self.draft = Some(after);
            return Ok(false);
        }
        let before = std::mem::replace(&mut self.records[index], after.clone());
        self.undo.push_back(Edit {
            before,
            after: after.clone(),
        });
        if self.undo.len() > MAX_HISTORY {
            self.undo.pop_front();
        }
        self.redo.clear();
        self.draft = Some(after);
        self.history_revision += 1;
        Ok(true)
    }

    pub fn cancel(&mut self) {
        let draft = self.selected_record().cloned();
        if self.draft != draft {
            self.draft_revision += 1;
        }
        self.draft = draft;
    }

    pub fn undo(&mut self) -> Result<bool, String> {
        self.require_clean_draft()?;
        let Some(edit) = self.undo.back() else {
            return Ok(false);
        };
        let index = self.record_index(edit.before.id)?;
        let edit = self.undo.pop_back().expect("the undo entry was inspected");
        self.records[index] = edit.before.clone();
        self.redo.push(edit);
        self.history_revision += 1;
        self.cancel();
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        self.require_clean_draft()?;
        let Some(edit) = self.redo.last() else {
            return Ok(false);
        };
        let index = self.record_index(edit.after.id)?;
        let edit = self.redo.pop().expect("the redo entry was inspected");
        self.records[index] = edit.after.clone();
        self.undo.push_back(edit);
        self.history_revision += 1;
        self.cancel();
        Ok(true)
    }

    fn selected_record(&self) -> Option<&Record> {
        self.records
            .iter()
            .find(|record| Some(record.id) == self.selected)
    }

    fn record_index(&self, id: RecordId) -> Result<usize, String> {
        self.records
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| "The history record does not exist.".to_owned())
    }

    fn require_clean_draft(&self) -> Result<(), String> {
        if self.is_dirty() {
            Err("Apply or cancel the draft before using undo or redo.".into())
        } else {
            Ok(())
        }
    }
}

impl Default for Desk {
    fn default() -> Self {
        let samples = [
            (
                1001,
                "Compare field notes",
                Category::Research,
                "Review the observations from the synthetic coastal survey.",
                false,
            ),
            (
                1013,
                "Prepare review agenda",
                Category::Operations,
                "Collect the open questions before the next team review.",
                true,
            ),
            (
                1027,
                "Try a compact summary",
                Category::Ideas,
                "Explore a one-page summary for a small collection.",
                false,
            ),
            (
                1049,
                "Check sampling assumptions",
                Category::Research,
                "Document the assumptions behind the example measurements.",
                true,
            ),
            (
                1061,
                "Refresh the handover checklist",
                Category::Operations,
                "Keep the checklist short and give each item a clear owner.",
                false,
            ),
            (
                1087,
                "Explore colour groupings",
                Category::Ideas,
                "Test whether category colours help people scan the list.",
                true,
            ),
            (
                1103,
                "Review interview themes",
                Category::Research,
                "Compare the fictional interview themes with the working questions.",
                false,
            ),
            (
                1129,
                "Schedule a maintenance window",
                Category::Operations,
                "Choose a quiet period for the example workflow review.",
                false,
            ),
            (
                1151,
                "Sketch a reading queue",
                Category::Ideas,
                "Make a small queue that preserves the current reading position.",
                false,
            ),
            (
                1171,
                "Summarise a pilot result",
                Category::Research,
                "Describe the synthetic pilot result and its limitations.",
                true,
            ),
            (
                1193,
                "Reconcile review actions",
                Category::Operations,
                "Mark completed actions and clarify the remaining next steps.",
                true,
            ),
            (
                1217,
                "Prototype a shared glossary",
                Category::Ideas,
                "Start with a few terms and a simple review process.",
                false,
            ),
        ];
        Self::new(
            samples
                .into_iter()
                .map(|(id, title, category, notes, reviewed)| Record {
                    id: RecordId(id),
                    title: title.into(),
                    category,
                    notes: notes.into(),
                    reviewed,
                })
                .collect(),
        )
        .expect("the authored synthetic records are valid")
    }
}

pub fn validate_record(record: &Record) -> Result<(), String> {
    if record.id.0 == 0 {
        return Err("Record IDs must be non-zero.".into());
    }
    let title = record.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err("A title must contain 1–120 characters after trimming.".into());
    }
    if record.title.chars().any(char::is_control) {
        return Err("Titles cannot contain control characters.".into());
    }
    if record.notes.chars().count() > 2000 {
        return Err("Notes must contain at most 2000 characters.".into());
    }
    if record
        .notes
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err("Notes allow line breaks and tabs, but no other control characters.".into());
    }
    Ok(())
}

fn validate_records(records: &[Record]) -> Result<(), String> {
    if records.len() > MAX_RECORDS {
        return Err(format!("Saved state allows at most {MAX_RECORDS} records."));
    }
    let mut ids = BTreeSet::new();
    for record in records {
        validate_record(record)?;
        if !ids.insert(record.id) {
            return Err(format!("Record ID {} occurs more than once.", record.id.0));
        }
    }
    Ok(())
}

pub fn default_workspace() -> Workspace {
    Workspace {
        schema_version: LAYOUT_SCHEMA_VERSION,
        root: DockNode::Split {
            id: DockNodeId(1),
            axis: SplitAxis::Horizontal,
            fraction: 0.36,
            first: Box::new(DockNode::Tabs {
                id: DockNodeId(2),
                tabs: vec![LIST_PANE],
                active: 0,
            }),
            second: Box::new(DockNode::Tabs {
                id: DockNodeId(3),
                tabs: vec![DETAIL_PANE],
                active: 0,
            }),
        },
        active_pane: LIST_PANE,
        closed_optional_panes: BTreeSet::new(),
        next_node_id: 4,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedState {
    pub schema_version: u32,
    pub records: Vec<Record>,
    pub workspace: Workspace,
}

impl SavedState {
    pub const VERSION: u32 = VERSION;

    pub fn from_desk(desk: &Desk, workspace: &Workspace) -> Self {
        Self {
            schema_version: VERSION,
            records: desk.records().to_vec(),
            workspace: workspace.clone(),
        }
    }

    pub fn decode(text: &str) -> Result<Self, String> {
        decode(text)
    }

    pub fn encode(&self) -> Result<String, String> {
        encode(self)
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema_version != VERSION {
            return Err(format!(
                "Unsupported Record Desk schema {} (expected {VERSION}).",
                self.schema_version
            ));
        }
        validate_records(&self.records)?;
        validate_workspace(&self.workspace)
    }
}

pub fn decode(text: &str) -> Result<SavedState, String> {
    if text.len() > MAX_STATE_BYTES {
        return Err("Saved state exceeds the 1 MiB limit.".into());
    }
    let state: SavedState = serde_json::from_str(text)
        .map_err(|error| format!("Saved state is not valid Record Desk JSON: {error}"))?;
    state.validate()?;
    Ok(state)
}

pub fn encode(state: &SavedState) -> Result<String, String> {
    state.validate()?;
    let text = serde_json::to_string_pretty(state)
        .map_err(|error| format!("Could not encode saved state: {error}"))?;
    if text.len() > MAX_STATE_BYTES {
        return Err("Saved state exceeds the 1 MiB limit.".into());
    }
    Ok(text)
}

fn validate_workspace(workspace: &Workspace) -> Result<(), String> {
    if !workspace.closed_optional_panes.is_empty() {
        return Err("Record Desk requires both panes to remain open.".into());
    }
    // Check bounds before the core's recursive identity validator. Do not repair
    // untrusted layouts by normalising them: a failed load must remain visible.
    let mut pending = vec![&workspace.root];
    let mut nodes = 0;
    let mut panes = Vec::new();
    while let Some(node) = pending.pop() {
        nodes += 1;
        if nodes > MAX_LAYOUT_NODES {
            return Err(format!(
                "Saved layout allows at most {MAX_LAYOUT_NODES} nodes."
            ));
        }
        match node {
            DockNode::Split {
                fraction,
                first,
                second,
                ..
            } => {
                if !fraction.is_finite() || !(0.1..=0.9).contains(fraction) {
                    return Err(
                        "Saved split fractions must be finite and between 0.1 and 0.9.".into(),
                    );
                }
                pending.push(first);
                pending.push(second);
            }
            DockNode::Tabs { tabs, active, .. } => {
                if tabs.is_empty() || *active >= tabs.len() {
                    return Err("Saved tab groups need a valid active tab.".into());
                }
                panes.extend(tabs.iter().copied());
            }
        }
    }
    panes.sort_unstable();
    if panes != [LIST_PANE, DETAIL_PANE] {
        return Err(
            "Saved layout must contain exactly the Record Desk list and detail panes.".into(),
        );
    }
    workspace
        .validate()
        .map_err(|error| format!("Invalid saved layout: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit_title(desk: &mut Desk, title: &str) {
        let mut draft = desk.draft().unwrap().clone();
        draft.title = title.into();
        desk.edit_draft(draft).unwrap();
    }

    #[test]
    fn empty_and_synthetic_desks_use_stable_ids() {
        let empty = Desk::new(vec![]).unwrap();
        assert_eq!(empty.selected(), None);
        assert_eq!(empty.draft(), None);
        assert!(empty.visible().is_empty());
        assert!(!empty.is_dirty());
        let desk = Desk::default();
        assert_eq!(desk.records().len(), 12);
        assert_eq!(desk.selected(), Some(RecordId(1001)));
        assert_eq!(desk.records()[1].id, RecordId(1013));
        assert_eq!(desk.records(), Desk::default().records());
    }

    #[test]
    fn invalid_drafts_never_mutate_committed_records_or_history() {
        let mut desk = Desk::default();
        let original = desk.records().to_vec();
        edit_title(&mut desk, "Valid edit");
        desk.apply().unwrap();
        desk.undo().unwrap();
        edit_title(&mut desk, " \t ");
        assert!(desk.apply().is_err());
        assert_eq!(desk.records(), original);
        assert_eq!((desk.undo_len(), desk.redo_len()), (0, 1));
        assert!(desk.is_dirty());
        assert!(desk.undo().is_err());
        assert!(desk.redo().is_err());
        assert!(desk.select(RecordId(1013)).is_err());
        assert!(desk.select(RecordId(1001)).is_ok());
        desk.cancel();
        assert!(!desk.is_dirty());
        assert_eq!(desk.draft(), original.first());
        assert_eq!((desk.undo_len(), desk.redo_len()), (0, 1));
    }

    #[test]
    fn one_apply_normalises_and_commits_every_field_as_one_transaction() {
        let mut desk = Desk::default();
        let original = desk.draft().unwrap().clone();
        let mut edited = original.clone();
        edited.title = "  Review the pilot  ".into();
        edited.notes = "  First observation\n\tSecond observation  ".into();
        edited.category = Category::Ideas;
        edited.reviewed = true;
        desk.edit_draft(edited).unwrap();
        assert!(desk.apply().unwrap());
        let committed = desk.draft().unwrap().clone();
        assert_eq!(committed.title, "Review the pilot");
        assert_eq!(committed.notes, "First observation\n\tSecond observation");
        assert_eq!(desk.undo_len(), 1);
        assert!(!desk.is_dirty());
        assert!(desk.undo().unwrap());
        assert_eq!(desk.draft(), Some(&original));
        assert!(desk.redo().unwrap());
        assert_eq!(desk.draft(), Some(&committed));
    }

    #[test]
    fn noop_apply_and_cancel_do_not_add_history_or_clear_redo() {
        let mut desk = Desk::default();
        assert!(!desk.apply().unwrap());
        edit_title(&mut desk, "Another title");
        desk.apply().unwrap();
        desk.undo().unwrap();
        let title = format!("  {}  ", desk.draft().unwrap().title);
        edit_title(&mut desk, &title);
        assert!(!desk.apply().unwrap());
        assert!(!desk.is_dirty());
        assert_eq!((desk.undo_len(), desk.redo_len()), (0, 1));
        edit_title(&mut desk, "Discard me");
        desk.cancel();
        assert_eq!((desk.undo_len(), desk.redo_len()), (0, 1));
        assert!(desk.redo().unwrap());
    }

    #[test]
    fn filters_keep_hidden_selection_without_a_phantom_row() {
        let mut desk = Desk::default();
        let original = desk.draft().unwrap().clone();
        desk.set_filters(Filters {
            query: "  AGENDA  ".into(),
            ..Filters::default()
        });
        assert_eq!(
            desk.visible()
                .iter()
                .map(|record| record.id)
                .collect::<Vec<_>>(),
            [RecordId(1013)]
        );
        assert_eq!(desk.selected(), Some(original.id));
        assert_eq!(desk.draft(), Some(&original));
        desk.set_filters(Filters {
            category: Some(Category::Research),
            review: ReviewFilter::Reviewed,
            query: "working questions".into(),
        });
        assert!(desk.visible().is_empty());
        desk.set_filters(Filters {
            query: "COASTAL SURVEY".into(),
            ..Filters::default()
        });
        assert_eq!(desk.visible().len(), 1);
        edit_title(&mut desk, "Draft retained when hidden");
        let draft = desk.draft().unwrap().clone();
        desk.set_filters(Filters {
            query: "nothing matches this text".into(),
            ..Filters::default()
        });
        assert!(desk.visible().is_empty());
        assert_eq!(desk.selected(), Some(original.id));
        assert_eq!(desk.draft(), Some(&draft));
        desk.set_filters(Filters::default());
        assert_eq!(desk.visible().len(), 12);
    }

    #[test]
    fn draft_identity_and_unknown_selection_are_rejected() {
        let mut desk = Desk::default();
        let original = desk.draft().unwrap().clone();
        let mut wrong = original.clone();
        wrong.id = RecordId(1013);
        assert!(desk.edit_draft(wrong).is_err());
        assert!(desk.select(RecordId(2)).is_err());
        assert_eq!(desk.draft(), Some(&original));
        let mut empty = Desk::new(vec![]).unwrap();
        assert!(empty.edit_draft(original).is_err());
        assert!(empty.apply().is_err());
    }

    #[test]
    fn history_is_bounded_and_a_new_branch_clears_redo() {
        let mut desk = Desk::default();
        for index in 0..105 {
            edit_title(&mut desk, &format!("Revision {index}"));
            desk.apply().unwrap();
        }
        assert_eq!(desk.undo_len(), MAX_HISTORY);
        for _ in 0..MAX_HISTORY {
            assert!(desk.undo().unwrap());
        }
        assert!(!desk.undo().unwrap());
        assert_eq!(desk.draft().unwrap().title, "Revision 4");
        assert_eq!(desk.redo_len(), MAX_HISTORY);
        desk.redo().unwrap();
        edit_title(&mut desk, "New branch");
        desk.apply().unwrap();
        assert_eq!(desk.redo_len(), 0);
        assert!(!desk.redo().unwrap());
    }

    #[test]
    fn undo_preserves_selection_and_refreshes_the_selected_draft() {
        let mut desk = Desk::default();
        let first = desk.draft().unwrap().clone();
        edit_title(&mut desk, "Changed first record");
        desk.apply().unwrap();
        desk.select(RecordId(1013)).unwrap();
        let selected = desk.draft().unwrap().clone();
        desk.undo().unwrap();
        assert_eq!(desk.selected(), Some(selected.id));
        assert_eq!(desk.draft(), Some(&selected));
        assert_eq!(desk.records().first(), Some(&first));
    }

    #[test]
    fn field_limits_count_characters_and_reject_control_characters() {
        let mut record = Desk::default().records()[0].clone();
        record.title = "é".repeat(120);
        record.notes = "界".repeat(2000);
        assert!(validate_record(&record).is_ok());
        record.title.push('é');
        assert!(validate_record(&record).is_err());
        record.title = "A title".into();
        record.notes.push('界');
        assert!(validate_record(&record).is_err());
        record.notes = "A line\n\tAnother line".into();
        assert!(validate_record(&record).is_ok());
        record.notes.push('\r');
        assert!(validate_record(&record).is_err());
        record.notes.clear();
        record.title.push('\n');
        assert!(validate_record(&record).is_err());
        record.title = "Good title".into();
        record.id = RecordId(0);
        assert!(validate_record(&record).is_err());
    }

    #[test]
    fn saved_roundtrip_excludes_drafts_filters_and_history() {
        let mut desk = Desk::default();
        edit_title(&mut desk, "A committed revision");
        desk.apply().unwrap();
        edit_title(&mut desk, "An unsaved draft");
        desk.set_filters(Filters {
            query: "no matches".into(),
            ..Filters::default()
        });
        let state = SavedState::from_desk(&desk, &default_workspace());
        let json = encode(&state).unwrap();
        assert!(!json.contains("An unsaved draft"));
        assert!(!json.contains("no matches"));
        assert_eq!(decode(&json).unwrap(), state);
        let restored = Desk::new(decode(&json).unwrap().records).unwrap();
        assert_eq!(restored.records(), desk.records());
        assert_eq!(restored.undo_len(), 0);
        assert_eq!(restored.redo_len(), 0);
        assert!(!restored.is_dirty());
    }

    #[test]
    fn malformed_unsupported_duplicate_zero_and_oversized_state_is_rejected() {
        assert!(decode("not JSON").is_err());
        assert!(decode(&" ".repeat(MAX_STATE_BYTES + 1)).is_err());
        let mut state = SavedState::from_desk(&Desk::default(), &default_workspace());
        state.schema_version += 1;
        assert!(encode(&state).is_err());
        assert!(decode(&serde_json::to_string(&state).unwrap()).is_err());
        state.schema_version = VERSION;
        state.records[1].id = state.records[0].id;
        assert!(encode(&state).is_err());
        assert!(Desk::new(state.records.clone()).is_err());
        state.records[1].id = RecordId(0);
        assert!(encode(&state).is_err());
        state.records = (1..=MAX_RECORDS + 1)
            .map(|id| Record {
                id: RecordId(id as u64),
                title: "Record".into(),
                category: Category::Ideas,
                notes: String::new(),
                reviewed: false,
            })
            .collect();
        assert!(encode(&state).is_err());
        state.records.truncate(MAX_RECORDS);
        for record in &mut state.records {
            record.notes = "界".repeat(2000);
        }
        assert!(encode(&state).is_err());
    }

    #[test]
    fn saved_layout_requires_valid_fractions_tabs_identities_and_open_panes() {
        let state = SavedState::from_desk(&Desk::default(), &default_workspace());
        for fraction in [f32::NAN, f32::INFINITY, -0.1, 0.0, 0.01, 0.99, 1.0] {
            let mut invalid = state.clone();
            let DockNode::Split {
                fraction: value, ..
            } = &mut invalid.workspace.root
            else {
                unreachable!()
            };
            *value = fraction;
            assert!(encode(&invalid).is_err(), "accepted fraction {fraction}");
        }
        let mut invalid = state.clone();
        let DockNode::Split { first, .. } = &mut invalid.workspace.root else {
            unreachable!()
        };
        let DockNode::Tabs { active, .. } = first.as_mut() else {
            unreachable!()
        };
        *active = 1;
        assert!(encode(&invalid).is_err());
        let mut invalid = state.clone();
        invalid.workspace.root = DockNode::Tabs {
            id: DockNodeId(1),
            tabs: vec![LIST_PANE, PaneId(999)],
            active: 0,
        };
        assert!(encode(&invalid).is_err());
        invalid.workspace.root = DockNode::Tabs {
            id: DockNodeId(1),
            tabs: vec![],
            active: 0,
        };
        assert!(encode(&invalid).is_err());
        let mut invalid = state.clone();
        invalid.workspace.closed_optional_panes.insert(DETAIL_PANE);
        assert!(encode(&invalid).is_err());
        let mut invalid = state.clone();
        invalid.workspace.next_node_id = 3;
        assert!(encode(&invalid).is_err());
        let mut invalid = state.clone();
        invalid.workspace.active_pane = PaneId(999);
        assert!(encode(&invalid).is_err());
        let mut invalid = state.clone();
        invalid.workspace.schema_version += 1;
        assert!(encode(&invalid).is_err());
        let mut deep = state;
        for id in 4..16 {
            deep.workspace.root = DockNode::Split {
                id: DockNodeId(id),
                axis: SplitAxis::Horizontal,
                fraction: 0.5,
                first: Box::new(deep.workspace.root),
                second: Box::new(DockNode::Tabs {
                    id: DockNodeId(id + 100),
                    tabs: vec![DETAIL_PANE],
                    active: 0,
                }),
            };
        }
        assert!(encode(&deep).unwrap_err().contains("12 nodes"));
    }
}
