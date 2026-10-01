//! Small native consumer of public framework APIs; no Lab application code.
//! Run and vary it using docs/application-composition.md.

use egui::{Color32, Pos2, Rect, Stroke, Ui};
use polyorama_core::{
    CommandHistory, DockNode, DockNodeId, Document, ImageIntent, LAYOUT_SCHEMA_VERSION, LayerId,
    PaneId, Session, Workspace, WorldPoint, validate_intent,
};
use polyorama_ui_egui::{
    ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionKey, ActionScope, ActionSpec,
    ActionTarget, Availability, DockBehaviour, DockTextContext, PanePresenter, PresentationContext,
    PresentationObservations, PresentationScope, SemanticUiId, UiPreferences, apply_design_system,
    dock_workspace,
};
use serde::Serialize;

const PANE: PaneId = PaneId(1);
const TITLE: &str = "Polyorama Minimal Workspace";

#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize)]
enum Action {
    AddTriangle,
}

impl ActionKey for Action {
    fn stable_id(self) -> &'static str {
        "minimal.add-triangle"
    }

    fn specification(self) -> ActionSpec<Self> {
        ActionSpec {
            id: self,
            label: "Add triangle",
            description: "Commit one project-authored polygon through a validated command",
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Pane,
        }
    }
}

// A narrow read model and intent sink are the entire pane interface.
struct TrianglePane<'a> {
    document: &'a Document,
    intents: &'a mut Vec<ImageIntent>,
    publication: &'a mut Option<PresentationObservations>,
    tokens: polyorama_ui_egui::DesignTokens,
}

impl PanePresenter for TrianglePane<'_> {
    fn title(&self, _pane: PaneId) -> &'static str {
        "Triangles"
    }

    fn pane_ui(&mut self, ui: &mut Ui, pane: PaneId, _pane_rect: Rect) {
        let mut presentation = PresentationContext::new(
            ui,
            self.tokens,
            1.0,
            PresentationScope::new(("minimal", pane)),
            SemanticUiId::root(),
        );
        presentation.heading(ui, "title", "A small public-API consumer");
        let add = presentation.action(
            ui,
            "add-triangle",
            ActionButtonSpec {
                target: ActionTarget::pane(Action::AddTriangle, pane),
                availability: Availability::Enabled,
                state: ActionButtonState::Momentary,
                emphasis: ActionEmphasis::Primary,
                compact: false,
            },
        );
        if add.clicked() {
            let offset = (self.document.annotations.len() % 5) as f64 * 0.06;
            self.intents.push(ImageIntent::CommitPolygon {
                layer: LayerId(1),
                vertices: vec![
                    WorldPoint::new(0.15 + offset, 0.75),
                    WorldPoint::new(0.45 + offset, 0.15),
                    WorldPoint::new(0.75 + offset, 0.75),
                ],
            });
        }
        presentation.property_row(
            ui,
            "count",
            "Committed triangles",
            &self.document.annotations.len().to_string(),
        );
        // Public variation point: add a measured status badge here.
        *self.publication = Some(presentation.finish(ui));

        let (canvas, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        let painter = ui.painter_at(canvas);
        for polygon in &self.document.annotations {
            let points = polygon
                .vertices
                .iter()
                .map(|point| {
                    Pos2::new(
                        canvas.left() + point.x as f32 * canvas.width(),
                        canvas.top() + point.y as f32 * canvas.height(),
                    )
                })
                .collect();
            painter.add(egui::Shape::convex_polygon(
                points,
                Color32::from_rgba_unmultiplied(72, 156, 220, 80),
                Stroke::new(2.0, Color32::from_rgb(72, 156, 220)),
            ));
        }
    }
}

struct MinimalApp {
    document: Document,
    session: Session,
    workspace: Workspace,
    history: CommandHistory,
    dock: DockBehaviour,
    tokens: polyorama_ui_egui::DesignTokens,
}

impl MinimalApp {
    fn new(creation: &eframe::CreationContext<'_>) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var_os("POLYORAMA_MINIMAL_SNAPSHOT").is_some()
            && let Some(render_state) = &creation.wgpu_render_state
        {
            eprintln!(
                "Minimal consumer adapter: {:?}",
                render_state.adapter.get_info()
            );
        }
        let preferences = UiPreferences::default();
        apply_design_system(&creation.egui_ctx, preferences);
        let workspace = Workspace {
            schema_version: LAYOUT_SCHEMA_VERSION,
            root: DockNode::Tabs {
                id: DockNodeId(1),
                tabs: vec![PANE],
                active: 0,
            },
            active_pane: PANE,
            closed_optional_panes: Default::default(),
            next_node_id: 2,
        };
        workspace.validate().expect("valid one-pane layout");
        Self {
            document: Document::default(),
            // This vector pane has no image camera or tool mapping.
            session: Session {
                cameras: vec![],
                active_tools: Default::default(),
                ..Session::default()
            },
            workspace,
            history: CommandHistory::default(),
            dock: DockBehaviour::default(),
            tokens: preferences.tokens(true),
        }
    }
}

impl eframe::App for MinimalApp {
    fn ui(&mut self, root: &mut Ui, _frame: &mut eframe::Frame) {
        let displayed_count = self.document.annotations.len();
        let mut intents = vec![];
        let mut publication = None;
        let mut presenter = TrianglePane {
            document: &self.document,
            intents: &mut intents,
            publication: &mut publication,
            tokens: self.tokens,
        };
        let dock_command = egui::CentralPanel::default()
            .show(root, |ui| {
                dock_workspace(
                    ui,
                    &mut self.workspace,
                    &mut self.dock,
                    &mut presenter,
                    DockTextContext {
                        tokens: self.tokens,
                        font_scale: 1.0,
                    },
                )
            })
            .inner;
        let mut commands: Vec<_> = dock_command.into_iter().collect();
        for intent in intents {
            commands.push(
                validate_intent(intent, &mut self.document, &self.session)
                    .expect("project-authored triangle is valid"),
            );
        }
        for command in commands {
            self.history.execute(
                command,
                &mut self.document,
                &mut self.session,
                &mut self.workspace,
            );
            // Recorded reason: a command changed the model after presentation.
            root.ctx().request_repaint();
        }
        if let Some(publication) = publication {
            write_smoke_observation(&self.document, &self.history, displayed_count, publication);
        }
    }
}

// Optional observations for the maintained physical native smoke. Normal runs
// create no files; this does not trigger repaint or automate application actions.
fn write_smoke_observation(
    document: &Document,
    history: &CommandHistory,
    displayed_count: usize,
    publication: PresentationObservations,
) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = std::env::var_os("POLYORAMA_MINIMAL_SNAPSHOT") {
        let snapshot = serde_json::json!({
            "annotations": document.annotations.len(),
            "undo_entries": history.undo_len(),
            "displayed_count": displayed_count,
            "nodes": publication.semantic_nodes,
            "text": publication.text_layouts,
            "text_audit": polyorama_ui_egui::audit_text_layouts(&publication.text_layouts),
        });
        std::fs::write(path, serde_json::to_vec_pretty(&snapshot).unwrap())
            .expect("write requested smoke observation");
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (document, history, displayed_count, publication);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    eframe::run_native(
        TITLE,
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 540.0]),
            ..Default::default()
        },
        Box::new(|creation| Ok(Box::new(MinimalApp::new(creation)))),
    )
}

// The portable consumer compiles on WASM. It supplies no browser bootstrap.
#[cfg(target_arch = "wasm32")]
fn main() {
    let _ = (TITLE, MinimalApp::new);
}
