use serde::{Deserialize, Serialize};

use crate::{
    AnnotationId, Camera, CameraChange, DockNodeId, Document, GesturePreview, LayerId, LinkGroupId,
    PaneId, Polygon, ResultId, Session, Workspace, WorldPoint, apply_camera_changes,
    linked_camera_changes, result_at,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ImageIntent {
    SetCamera {
        pane: PaneId,
        camera: Camera,
    },
    SetCameraLink {
        pane: PaneId,
        link: Option<LinkGroupId>,
    },
    CommitPolygon {
        layer: LayerId,
        vertices: Vec<WorldPoint>,
    },
    MoveVertex {
        annotation: AnnotationId,
        vertex: usize,
        to: WorldPoint,
    },
    DeleteAnnotation {
        annotation: AnnotationId,
    },
    SelectResult {
        result: ResultId,
    },
    RecenterOnResult {
        result: ResultId,
        pane: PaneId,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    SetCameras {
        changes: Vec<CameraChange>,
    },
    SetCameraLink {
        pane: PaneId,
        before: Option<LinkGroupId>,
        after: Option<LinkGroupId>,
        before_camera: Camera,
        after_camera: Camera,
    },
    AddPolygon {
        polygon: Polygon,
    },
    MoveVertex {
        annotation: AnnotationId,
        vertex: usize,
        before: WorldPoint,
        after: WorldPoint,
    },
    DeletePolygon {
        polygon: Polygon,
        index: usize,
    },
    SelectResult {
        before: Option<ResultId>,
        after: Option<ResultId>,
    },
    ResizeSplit {
        node: DockNodeId,
        before: f32,
        after: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoScope {
    Document,
    View,
    Selection,
    Workspace,
}

impl Command {
    pub fn undo_scope(&self) -> UndoScope {
        match self {
            Self::AddPolygon { .. } | Self::MoveVertex { .. } | Self::DeletePolygon { .. } => {
                UndoScope::Document
            }
            Self::SetCameras { .. } | Self::SetCameraLink { .. } => UndoScope::View,
            Self::SelectResult { .. } => UndoScope::Selection,
            Self::ResizeSplit { .. } => UndoScope::Workspace,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CommandHistory {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl CommandHistory {
    pub fn execute(
        &mut self,
        command: Command,
        document: &mut Document,
        session: &mut Session,
        workspace: &mut Workspace,
    ) {
        if matches!(
            &command,
            Command::SetCameras { changes }
                if changes.is_empty() || changes.iter().all(|change| change.before == change.after)
        ) {
            return;
        }
        apply(&command, document, session, workspace);
        self.undo.push(command);
        self.redo.clear();
    }

    pub fn undo(
        &mut self,
        document: &mut Document,
        session: &mut Session,
        workspace: &mut Workspace,
    ) -> bool {
        let Some(command) = self.undo.pop() else {
            return false;
        };
        revert(&command, document, session, workspace);
        self.redo.push(command);
        true
    }

    pub fn redo(
        &mut self,
        document: &mut Document,
        session: &mut Session,
        workspace: &mut Workspace,
    ) -> bool {
        let Some(command) = self.redo.pop() else {
            return false;
        };
        apply(&command, document, session, workspace);
        self.undo.push(command);
        true
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
}

pub fn validate_intent(
    intent: ImageIntent,
    document: &mut Document,
    session: &Session,
) -> Result<Command, String> {
    match intent {
        ImageIntent::SetCamera { pane, camera } => {
            if !session.cameras.iter().any(|state| state.pane == pane) {
                return Err("unknown camera pane".into());
            }
            Ok(Command::SetCameras {
                changes: linked_camera_changes(&session.cameras, pane, camera),
            })
        }
        ImageIntent::SetCameraLink { pane, link } => {
            let state = session
                .cameras
                .iter()
                .find(|state| state.pane == pane)
                .ok_or("unknown camera pane")?;
            let after_camera = link
                .and_then(|group| {
                    session
                        .cameras
                        .iter()
                        .find(|candidate| candidate.pane != pane && candidate.link == Some(group))
                })
                .map_or(state.camera, |candidate| candidate.camera);
            Ok(Command::SetCameraLink {
                pane,
                before: state.link,
                after: link,
                before_camera: state.camera,
                after_camera,
            })
        }
        ImageIntent::CommitPolygon { layer, vertices } => {
            if vertices.len() < 3 {
                return Err("a polygon needs at least three vertices".into());
            }
            let id = AnnotationId(document.next_annotation_id);
            document.next_annotation_id += 1;
            Ok(Command::AddPolygon {
                polygon: Polygon {
                    id,
                    layer,
                    vertices,
                },
            })
        }
        ImageIntent::MoveVertex {
            annotation,
            vertex,
            to,
        } => {
            let polygon = document
                .annotations
                .iter()
                .find(|polygon| polygon.id == annotation)
                .ok_or("unknown annotation")?;
            let before = *polygon.vertices.get(vertex).ok_or("unknown vertex")?;
            Ok(Command::MoveVertex {
                annotation,
                vertex,
                before,
                after: to,
            })
        }
        ImageIntent::DeleteAnnotation { annotation } => {
            let index = document
                .annotations
                .iter()
                .position(|polygon| polygon.id == annotation)
                .ok_or("unknown annotation")?;
            Ok(Command::DeletePolygon {
                polygon: document.annotations[index].clone(),
                index,
            })
        }
        ImageIntent::SelectResult { result } => Ok(Command::SelectResult {
            before: session.selected_result,
            after: Some(result),
        }),
        ImageIntent::RecenterOnResult { result, pane } => {
            let mut after = session
                .cameras
                .iter()
                .find(|state| state.pane == pane)
                .ok_or("unknown camera pane")?
                .camera;
            after.centre = result_at(result.0).position;
            Ok(Command::SetCameras {
                changes: linked_camera_changes(&session.cameras, pane, after),
            })
        }
    }
}

pub fn commit_gesture(document: &mut Document, session: &mut Session) -> Result<Command, String> {
    match session.gesture.take().ok_or("no active gesture")? {
        GesturePreview::Polygon { layer, vertices } => validate_intent(
            ImageIntent::CommitPolygon { layer, vertices },
            document,
            session,
        ),
        GesturePreview::Vertex {
            annotation,
            vertex,
            original,
            preview,
        } => Ok(Command::MoveVertex {
            annotation,
            vertex,
            before: original,
            after: preview,
        }),
    }
}

fn apply(
    command: &Command,
    document: &mut Document,
    session: &mut Session,
    workspace: &mut Workspace,
) {
    match command {
        Command::SetCameras { changes } => {
            apply_camera_changes(&mut session.cameras, changes, true)
        }
        Command::SetCameraLink {
            pane,
            after,
            after_camera,
            ..
        } => {
            if let Some(camera) = session.cameras.iter_mut().find(|state| state.pane == *pane) {
                camera.link = *after;
                camera.camera = *after_camera;
            }
        }
        Command::AddPolygon { polygon } => {
            document.annotations.push(polygon.clone());
            session.selected_annotation = Some(polygon.id);
        }
        Command::MoveVertex {
            annotation,
            vertex,
            after,
            ..
        } => {
            if let Some(polygon) = document
                .annotations
                .iter_mut()
                .find(|item| item.id == *annotation)
            {
                polygon.vertices[*vertex] = *after;
            }
        }
        Command::DeletePolygon { polygon, .. } => {
            document.annotations.retain(|item| item.id != polygon.id);
            if session.selected_annotation == Some(polygon.id) {
                session.selected_annotation = None;
            }
        }
        Command::SelectResult { after, .. } => session.selected_result = *after,
        Command::ResizeSplit { node, after, .. } => {
            workspace.root.set_split_fraction(*node, *after);
        }
    }
}

fn revert(
    command: &Command,
    document: &mut Document,
    session: &mut Session,
    workspace: &mut Workspace,
) {
    match command {
        Command::SetCameras { changes } => {
            apply_camera_changes(&mut session.cameras, changes, false)
        }
        Command::SetCameraLink {
            pane,
            before,
            before_camera,
            ..
        } => {
            if let Some(camera) = session.cameras.iter_mut().find(|state| state.pane == *pane) {
                camera.link = *before;
                camera.camera = *before_camera;
            }
        }
        Command::AddPolygon { polygon } => {
            document.annotations.retain(|item| item.id != polygon.id);
            session.selected_annotation = None;
        }
        Command::MoveVertex {
            annotation,
            vertex,
            before,
            ..
        } => {
            if let Some(polygon) = document
                .annotations
                .iter_mut()
                .find(|item| item.id == *annotation)
            {
                polygon.vertices[*vertex] = *before;
            }
        }
        Command::DeletePolygon { polygon, index } => {
            document
                .annotations
                .insert((*index).min(document.annotations.len()), polygon.clone());
            session.selected_annotation = Some(polygon.id);
        }
        Command::SelectResult { before, .. } => session.selected_result = *before,
        Command::ResizeSplit { node, before, .. } => {
            workspace.root.set_split_fraction(*node, *before);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GesturePreview, LayerId};

    #[test]
    fn command_application_undo_and_redo() {
        let mut document = Document::default();
        let mut session = Session::default();
        let mut workspace = Workspace::analytical_default();
        let command = validate_intent(
            ImageIntent::CommitPolygon {
                layer: LayerId(1),
                vertices: vec![
                    WorldPoint::new(0.0, 0.0),
                    WorldPoint::new(1.0, 0.0),
                    WorldPoint::new(0.0, 1.0),
                ],
            },
            &mut document,
            &session,
        )
        .unwrap();
        let mut history = CommandHistory::default();
        history.execute(command, &mut document, &mut session, &mut workspace);
        assert_eq!(document.annotations.len(), 1);
        assert!(history.undo(&mut document, &mut session, &mut workspace));
        assert!(document.annotations.is_empty());
        assert!(history.redo(&mut document, &mut session, &mut workspace));
        assert_eq!(document.annotations.len(), 1);
    }

    #[test]
    fn complete_gesture_is_one_command() {
        let mut document = Document::default();
        let mut session = Session {
            gesture: Some(GesturePreview::Polygon {
                layer: LayerId(1),
                vertices: vec![
                    WorldPoint::new(0.0, 0.0),
                    WorldPoint::new(1.0, 0.0),
                    WorldPoint::new(0.0, 1.0),
                ],
            }),
            ..Session::default()
        };
        let mut workspace = Workspace::analytical_default();
        let command = commit_gesture(&mut document, &mut session).unwrap();
        let mut history = CommandHistory::default();
        history.execute(command, &mut document, &mut session, &mut workspace);
        assert_eq!(history.undo_len(), 1);
    }

    #[test]
    fn invalid_polygon_intent_does_not_become_a_command() {
        let mut document = Document::default();
        let session = Session::default();
        let result = validate_intent(
            ImageIntent::CommitPolygon {
                layer: LayerId(1),
                vertices: vec![WorldPoint::new(0.0, 0.0), WorldPoint::new(1.0, 0.0)],
            },
            &mut document,
            &session,
        );
        assert_eq!(
            result,
            Err("a polygon needs at least three vertices".into())
        );
        assert_eq!(document, Document::default());
    }

    #[test]
    fn result_selection_is_stable_across_virtual_ranges_and_undoable() {
        let mut document = Document::default();
        let mut session = Session::default();
        let mut workspace = Workspace::analytical_default();
        let mut history = CommandHistory::default();
        let command = validate_intent(
            ImageIntent::SelectResult {
                result: ResultId(734_219),
            },
            &mut document,
            &session,
        )
        .unwrap();

        history.execute(command, &mut document, &mut session, &mut workspace);
        assert_eq!(session.selected_result, Some(ResultId(734_219)));
        let range = crate::virtual_rows(700_000.0, 480.0, 20.0, 1_000_000, 8);
        assert_eq!(range.visible.start, 35_000);
        assert_eq!(range.materialised.start, 34_992);
        assert_eq!(session.selected_result, Some(ResultId(734_219)));
        assert!(history.undo(&mut document, &mut session, &mut workspace));
        assert_eq!(session.selected_result, None);
        assert!(history.redo(&mut document, &mut session, &mut workspace));
        assert_eq!(session.selected_result, Some(ResultId(734_219)));
    }

    #[test]
    fn splitter_gesture_is_one_undoable_workspace_command() {
        let mut document = Document::default();
        let mut session = Session::default();
        let mut workspace = Workspace::analytical_default();
        let node = DockNodeId(1);
        let before = workspace.root.split_fraction(node).unwrap();
        let mut history = CommandHistory::default();

        history.execute(
            Command::ResizeSplit {
                node,
                before,
                after: 0.61,
            },
            &mut document,
            &mut session,
            &mut workspace,
        );

        assert_eq!(history.undo_len(), 1);
        assert_eq!(workspace.root.split_fraction(node), Some(0.61));
        assert!(history.undo(&mut document, &mut session, &mut workspace));
        assert_eq!(workspace.root.split_fraction(node), Some(before));
        assert!(history.redo(&mut document, &mut session, &mut workspace));
        assert_eq!(workspace.root.split_fraction(node), Some(0.61));
    }

    #[test]
    fn linked_camera_undo_restores_exact_snapshot_after_topology_changes() {
        let mut document = Document::default();
        let mut session = Session::default();
        let mut workspace = Workspace::analytical_default();
        session.cameras[1].camera.centre.x = 17.0;
        let original = session.cameras.clone();
        let mut updated = session.cameras[0].camera;
        updated.centre.x = 91.0;
        let command = validate_intent(
            ImageIntent::SetCamera {
                pane: PaneId(1),
                camera: updated,
            },
            &mut document,
            &session,
        )
        .unwrap();
        let mut history = CommandHistory::default();

        history.execute(command, &mut document, &mut session, &mut workspace);
        assert_eq!(session.cameras[0].camera, updated);
        assert_eq!(session.cameras[1].camera, updated);
        session.cameras[1].link = None;

        assert!(history.undo(&mut document, &mut session, &mut workspace));
        assert_eq!(session.cameras[0].camera, original[0].camera);
        assert_eq!(session.cameras[1].camera, original[1].camera);
        assert_eq!(session.cameras[1].link, None);
        assert!(history.redo(&mut document, &mut session, &mut workspace));
        assert_eq!(session.cameras[0].camera, updated);
        assert_eq!(session.cameras[1].camera, updated);
        assert_eq!(session.cameras[1].link, None);
    }

    #[test]
    fn no_op_camera_command_is_not_recorded() {
        let mut document = Document::default();
        let mut session = Session::default();
        let original = session.cameras.clone();
        let mut workspace = Workspace::analytical_default();
        let mut history = CommandHistory::default();

        history.execute(
            Command::SetCameras {
                changes: vec![CameraChange {
                    pane: PaneId(1),
                    before: original[0].camera,
                    after: original[0].camera,
                }],
            },
            &mut document,
            &mut session,
            &mut workspace,
        );

        assert_eq!(session.cameras, original);
        assert_eq!(history.undo_len(), 0);
        assert!(!history.undo(&mut document, &mut session, &mut workspace));
    }

    #[test]
    fn joining_camera_link_synchronises_and_undo_restores_camera() {
        let mut document = Document::default();
        let mut session = Session::default();
        let mut workspace = Workspace::analytical_default();
        session.cameras[2].camera.centre.x = 8_192.0;
        let before = session.cameras[2].camera;
        let linked = session.cameras[0].camera;
        let command = validate_intent(
            ImageIntent::SetCameraLink {
                pane: PaneId(3),
                link: Some(LinkGroupId(1)),
            },
            &mut document,
            &session,
        )
        .unwrap();
        let mut history = CommandHistory::default();

        history.execute(command, &mut document, &mut session, &mut workspace);
        assert_eq!(session.cameras[2].link, Some(LinkGroupId(1)));
        assert_eq!(session.cameras[2].camera, linked);
        assert!(history.undo(&mut document, &mut session, &mut workspace));
        assert_eq!(session.cameras[2].link, None);
        assert_eq!(session.cameras[2].camera, before);
    }

    #[test]
    fn command_undo_scope_is_explicit() {
        assert_eq!(
            Command::SelectResult {
                before: None,
                after: Some(ResultId(1)),
            }
            .undo_scope(),
            UndoScope::Selection
        );
        assert_eq!(
            Command::ResizeSplit {
                node: DockNodeId(1),
                before: 0.5,
                after: 0.6,
            }
            .undo_scope(),
            UndoScope::Workspace
        );
    }
}
