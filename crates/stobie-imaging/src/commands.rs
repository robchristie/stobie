use serde::{Deserialize, Serialize};

use stobie_core::{Command, PaneId};

use crate::{
    AnnotationId, Camera, CameraChange, Document, GesturePreview, ImagePoint, LayerId, LinkGroupId,
    Polygon, ResultId, Session, WorldPoint, apply_camera_changes, linked_camera_changes,
};

#[derive(Clone, Debug, PartialEq)]
/// Feature requests emitted by UI code and resolved by [`validate_intent`].
/// An intent is not yet an undoable mutation; validation uses current application state.
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
    /// Centre a pane's camera, and the cameras linked to it, on an image point.
    Recentre {
        pane: PaneId,
        centre: ImagePoint,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ImageCommand {
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
}

/// The image state an [`ImageCommand`] changes: a borrow of the document and
/// the session, built where the command is executed.
pub struct ImageTarget<'a> {
    pub document: &'a mut Document,
    pub session: &'a mut Session,
}

impl<'a> ImageTarget<'a> {
    pub fn new(document: &'a mut Document, session: &'a mut Session) -> Self {
        Self { document, session }
    }
}

/// Image commands apply already validated snapshots without revalidating them;
/// do not execute fabricated or stale commands that violate current annotation
/// or camera mappings. Empty or unchanged camera commands change nothing, so a
/// [`History`](stobie_core::History) records no entry for them. Reverting a
/// change to a camera or annotation that no longer exists does nothing.
impl Command<ImageTarget<'_>> for ImageCommand {
    fn apply(&self, target: &mut ImageTarget<'_>) -> Result<bool, String> {
        if let Self::SetCameras { changes } = self
            && changes.iter().all(|change| change.before == change.after)
        {
            return Ok(false);
        }
        apply(self, target.document, target.session);
        Ok(true)
    }

    fn revert(&self, target: &mut ImageTarget<'_>) -> Result<(), String> {
        revert(self, target.document, target.session);
        Ok(())
    }
}

/// Resolve a feature intent against current state into an undoable command.
///
/// Successful polygon validation reserves an ID by advancing
/// [`Document::next_annotation_id`] before execution; discarding the returned
/// command leaves that ID unused. Other supported intents snapshot relevant
/// current state. Execute a returned command before unrelated mutations can
/// invalidate those snapshots. Validation is specific to each variant, not a
/// general geometry/layer/result-ID validator. Errors are human-readable strings;
/// callers should not depend on wording or infer universal validation precedence.
pub fn validate_intent(
    intent: ImageIntent,
    document: &mut Document,
    session: &Session,
) -> Result<ImageCommand, String> {
    match intent {
        ImageIntent::SetCamera { pane, camera } => {
            if !session.cameras.iter().any(|state| state.pane == pane) {
                return Err("unknown camera pane".into());
            }
            Ok(ImageCommand::SetCameras {
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
            Ok(ImageCommand::SetCameraLink {
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
            Ok(ImageCommand::AddPolygon {
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
            Ok(ImageCommand::MoveVertex {
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
            Ok(ImageCommand::DeletePolygon {
                polygon: document.annotations[index].clone(),
                index,
            })
        }
        ImageIntent::SelectResult { result } => Ok(ImageCommand::SelectResult {
            before: session.selected_result,
            after: Some(result),
        }),
        ImageIntent::Recentre { pane, centre } => {
            let mut after = session
                .cameras
                .iter()
                .find(|state| state.pane == pane)
                .ok_or("unknown camera pane")?
                .camera;
            after.centre = centre;
            Ok(ImageCommand::SetCameras {
                changes: linked_camera_changes(&session.cameras, pane, after),
            })
        }
    }
}

pub fn commit_gesture(
    document: &mut Document,
    session: &mut Session,
) -> Result<ImageCommand, String> {
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
        } => Ok(ImageCommand::MoveVertex {
            annotation,
            vertex,
            before: original,
            after: preview,
        }),
    }
}

fn apply(command: &ImageCommand, document: &mut Document, session: &mut Session) {
    match command {
        ImageCommand::SetCameras { changes } => {
            apply_camera_changes(&mut session.cameras, changes, true)
        }
        ImageCommand::SetCameraLink {
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
        ImageCommand::AddPolygon { polygon } => {
            document.annotations.push(polygon.clone());
            session.selected_annotation = Some(polygon.id);
        }
        ImageCommand::MoveVertex {
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
        ImageCommand::DeletePolygon { polygon, .. } => {
            document.annotations.retain(|item| item.id != polygon.id);
            if session.selected_annotation == Some(polygon.id) {
                session.selected_annotation = None;
            }
        }
        ImageCommand::SelectResult { after, .. } => session.selected_result = *after,
    }
}

fn revert(command: &ImageCommand, document: &mut Document, session: &mut Session) {
    match command {
        ImageCommand::SetCameras { changes } => {
            apply_camera_changes(&mut session.cameras, changes, false)
        }
        ImageCommand::SetCameraLink {
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
        ImageCommand::AddPolygon { polygon } => {
            document.annotations.retain(|item| item.id != polygon.id);
            session.selected_annotation = None;
        }
        ImageCommand::MoveVertex {
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
        ImageCommand::DeletePolygon { polygon, index } => {
            document
                .annotations
                .insert((*index).min(document.annotations.len()), polygon.clone());
            session.selected_annotation = Some(polygon.id);
        }
        ImageCommand::SelectResult { before, .. } => session.selected_result = *before,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CameraState, GesturePreview, LayerId};
    use stobie_core::History;

    /// Four image panes; the first two share a camera link.
    fn session() -> Session {
        Session {
            cameras: (1..=4)
                .map(|id| CameraState {
                    pane: PaneId(id),
                    camera: Camera::default(),
                    link: (id <= 2).then_some(LinkGroupId(1)),
                })
                .collect(),
            ..Session::default()
        }
    }

    #[test]
    fn command_application_undo_and_redo() {
        let mut document = Document::default();
        let mut session = session();
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
        let mut history = History::default();
        history
            .execute(command, &mut ImageTarget::new(&mut document, &mut session))
            .unwrap();
        assert_eq!(document.annotations.len(), 1);
        assert!(
            history
                .undo(&mut ImageTarget::new(&mut document, &mut session))
                .unwrap()
        );
        assert!(document.annotations.is_empty());
        assert!(
            history
                .redo(&mut ImageTarget::new(&mut document, &mut session))
                .unwrap()
        );
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
            ..session()
        };
        let command = commit_gesture(&mut document, &mut session).unwrap();
        let mut history = History::default();
        history
            .execute(command, &mut ImageTarget::new(&mut document, &mut session))
            .unwrap();
        assert_eq!(history.undo_len(), 1);
    }

    #[test]
    fn invalid_polygon_intent_does_not_become_a_command() {
        let mut document = Document::default();
        let session = session();
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
        let mut session = session();
        let mut history = History::default();
        let command = validate_intent(
            ImageIntent::SelectResult {
                result: ResultId(734_219),
            },
            &mut document,
            &session,
        )
        .unwrap();

        history
            .execute(command, &mut ImageTarget::new(&mut document, &mut session))
            .unwrap();
        assert_eq!(session.selected_result, Some(ResultId(734_219)));
        let range = stobie_core::virtual_rows(700_000.0, 480.0, 20.0, 1_000_000, 8);
        assert_eq!(range.visible.start, 35_000);
        assert_eq!(range.materialised.start, 34_992);
        assert_eq!(session.selected_result, Some(ResultId(734_219)));
        assert!(
            history
                .undo(&mut ImageTarget::new(&mut document, &mut session))
                .unwrap()
        );
        assert_eq!(session.selected_result, None);
        assert!(
            history
                .redo(&mut ImageTarget::new(&mut document, &mut session))
                .unwrap()
        );
        assert_eq!(session.selected_result, Some(ResultId(734_219)));
    }

    #[test]
    fn linked_camera_undo_restores_exact_snapshot_after_topology_changes() {
        let mut document = Document::default();
        let mut session = session();
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
        let mut history = History::default();

        history
            .execute(command, &mut ImageTarget::new(&mut document, &mut session))
            .unwrap();
        assert_eq!(session.cameras[0].camera, updated);
        assert_eq!(session.cameras[1].camera, updated);
        session.cameras[1].link = None;

        assert!(
            history
                .undo(&mut ImageTarget::new(&mut document, &mut session))
                .unwrap()
        );
        assert_eq!(session.cameras[0].camera, original[0].camera);
        assert_eq!(session.cameras[1].camera, original[1].camera);
        assert_eq!(session.cameras[1].link, None);
        assert!(
            history
                .redo(&mut ImageTarget::new(&mut document, &mut session))
                .unwrap()
        );
        assert_eq!(session.cameras[0].camera, updated);
        assert_eq!(session.cameras[1].camera, updated);
        assert_eq!(session.cameras[1].link, None);
    }

    #[test]
    fn no_op_camera_command_is_not_recorded() {
        let mut document = Document::default();
        let mut session = session();
        let original = session.cameras.clone();
        let mut history = History::default();

        history
            .execute(
                ImageCommand::SetCameras {
                    changes: vec![CameraChange {
                        pane: PaneId(1),
                        before: original[0].camera,
                        after: original[0].camera,
                    }],
                },
                &mut ImageTarget::new(&mut document, &mut session),
            )
            .unwrap();

        assert_eq!(session.cameras, original);
        assert_eq!(history.undo_len(), 0);
        assert!(
            !history
                .undo(&mut ImageTarget::new(&mut document, &mut session))
                .unwrap()
        );
    }

    #[test]
    fn joining_camera_link_synchronises_and_undo_restores_camera() {
        let mut document = Document::default();
        let mut session = session();
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
        let mut history = History::default();

        history
            .execute(command, &mut ImageTarget::new(&mut document, &mut session))
            .unwrap();
        assert_eq!(session.cameras[2].link, Some(LinkGroupId(1)));
        assert_eq!(session.cameras[2].camera, linked);
        assert!(
            history
                .undo(&mut ImageTarget::new(&mut document, &mut session))
                .unwrap()
        );
        assert_eq!(session.cameras[2].link, None);
        assert_eq!(session.cameras[2].camera, before);
    }
}
