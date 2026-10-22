//! The navigation recipe: a list, the item opened from it and that item's
//! detail, side by side when the width allows and stacked with Back when it
//! does not.

use std::hash::Hash;

use egui::{Align, Layout, Rect, Response, Stroke, Ui, UiBuilder, pos2, vec2};

use super::heading::{RowMetrics, minimum_title_width, plan_row};
use super::{
    ContentTextSpec, HeaderControl, HeaderControlResponse, PresentationContext,
    PresentationObservations, PresentationScope,
};
use crate::{
    ActionArgument, ActionButtonContent, ActionButtonSpec, ActionButtonState, ActionEmphasis,
    ActionKey, ActionTarget, Availability, AvatarSpec, DomainReference, IconId, IntoActionTarget,
    SemanticUiId, TextInteraction, TextOverflow, TextRole, TextRoleStyles, UiNode, UiRole,
    components::action_button_size,
    components::icon_action_side,
    focus_return::{self, FocusReturn},
};

/// How a navigation recipe arranges its panes at the width it is given.
///
/// The breakpoints are typed policy in layout points, the same points at
/// every text size: below [`Self::SPLIT_FROM`] the panes stack, from it the
/// list stands beside the content, and from [`Self::COLUMNS_FROM`] the
/// detail stands beside both. Each breakpoint is the first width at which
/// every pane it shows keeps its own width, so between breakpoints the
/// content takes whatever is left and nothing is squeezed below its
/// minimum.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NavigationLayout {
    /// One level at a time, the deepest open one, under a header with Back:
    /// the list, then the open item, then its detail.
    Stack,
    /// The list beside the open item. A shown detail takes the item's place,
    /// under a header with Back.
    Split,
    /// The list, the open item and its shown detail side by side.
    Columns,
}

impl NavigationLayout {
    /// The list pane's width beside other panes.
    pub const LIST_WIDTH: f32 = 320.0;
    /// The detail pane's width beside the content.
    pub const DETAIL_WIDTH: f32 = 320.0;
    /// The least width the content keeps beside other panes.
    pub const CONTENT_MIN_WIDTH: f32 = 400.0;
    /// The first width with the list beside the content: 720 points, where
    /// [`PaneWidthClass::Wide`](crate::PaneWidthClass::Wide) starts.
    pub const SPLIT_FROM: f32 = Self::LIST_WIDTH + Self::CONTENT_MIN_WIDTH;
    /// The first width with the detail beside the content too: 1040 points.
    pub const COLUMNS_FROM: f32 = Self::SPLIT_FROM + Self::DETAIL_WIDTH;

    /// The layout for a recipe given `width` points.
    pub fn for_width(width: f32) -> Self {
        if width >= Self::COLUMNS_FROM {
            Self::Columns
        } else if width >= Self::SPLIT_FROM {
            Self::Split
        } else {
            Self::Stack
        }
    }

    /// The layout's name in observations: `stack`, `split` or `columns`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stack => "stack",
            Self::Split => "split",
            Self::Columns => "columns",
        }
    }
}

/// Which part of a navigation recipe its closure is asked to present.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NavigationPane {
    /// The list of destinations, such as conversations.
    List,
    /// The open item's content, such as one conversation.
    Content,
    /// The open item's detail, such as the person's profile.
    Detail,
    /// The content's place while nothing is open, in the split and column
    /// layouts: present a short prompt, such as "Choose a conversation". It
    /// has no header and is not a region.
    Empty,
}

impl NavigationPane {
    fn key(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Content => "content",
            Self::Detail => "detail",
            Self::Empty => "empty",
        }
    }
}

/// The open item's detail: a further level shown beside the content or
/// pushed over it.
#[derive(Clone, Copy, Debug)]
pub struct NavigationDetail<'a, A> {
    /// The detail pane's title, such as "About Ada".
    pub title: &'a str,
    /// Whether the application shows the detail.
    pub shown: bool,
    /// The action that shows or hides the detail, presented as a toggle in
    /// the content's header and named by its label.
    pub toggle: A,
    /// The toggle's icon.
    pub icon: IconId,
}

/// The item open from the list.
#[derive(Clone, Debug)]
pub struct NavigationContent<'a, A> {
    /// The content pane's title, such as the conversation's name.
    pub title: &'a str,
    /// The open item's domain identity, published on the content pane so
    /// agents can tell which item is open.
    pub reference: Option<DomainReference>,
    /// The item's detail, if it has one.
    pub detail: Option<NavigationDetail<'a, A>>,
}

/// What a navigation recipe shows. The application owns all of it: which
/// item is open, whether its detail is shown, and the actions that change
/// them.
///
/// ```
/// use serde::Serialize;
/// use stobie_ui_egui::{
///     Action, ActionArgument, ActionTarget, Application, ApplicationIdentity, Availability,
///     BoundAction, DestinationSpec, DomainReference, Frame, IconId, NavigationContent,
///     NavigationDetail, NavigationPane, NavigationSpec, PresentationScope, SemanticUiId,
///     application_identity,
/// };
///
/// #[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
/// #[action(prefix = "chat")]
/// enum ChatAction {
///     #[action(label = "Open", description = "Open the conversation with this ID",
///              argument = "text", max_length = 16)]
///     Open,
///     #[action(label = "Back", description = "Return to the previous level")]
///     Back,
///     #[action(label = "Details", description = "Show or hide the details")]
///     Details,
/// }
///
/// struct Conversation { id: String, name: String }
/// struct Chat { conversations: Vec<Conversation>, open: Option<usize>, details: bool }
///
/// impl Application for Chat {
///     type Action = ChatAction;
///     type Intent = ();
///     type UiState = ();
///
///     fn identity(&self) -> ApplicationIdentity {
///         application_identity!("Chat")
///     }
///
///     // Back and the toggle are hidden while nothing is open.
///     fn availability(&self, target: ActionTarget<ChatAction>) -> Availability {
///         match target.action {
///             ChatAction::Open => Availability::Enabled,
///             _ if self.open.is_some() => Availability::Enabled,
///             _ => Availability::Hidden,
///         }
///     }
///
///     fn ui(&self, ui: &mut egui::Ui, _: &mut (), frame: &mut Frame<'_, ChatAction, ()>) {
///         let mut p = frame.presentation(ui, PresentationScope::new("chat"), SemanticUiId::root());
///         let open = self.open.map(|index| &self.conversations[index]);
///         let spec = NavigationSpec {
///             list_title: "Conversations",
///             content: open.map(|conversation| NavigationContent {
///                 title: &conversation.name,
///                 reference: Some(DomainReference::External {
///                     namespace: "conversation".into(),
///                     id: conversation.id.clone(),
///                 }),
///                 detail: Some(NavigationDetail {
///                     title: "Details",
///                     shown: self.details,
///                     toggle: ChatAction::Details,
///                     icon: IconId::Info,
///                 }),
///             }),
///             back: ChatAction::Back,
///         };
///         frame.navigation(ui, &mut p, "conversations", &spec, |ui, p, frame, pane| {
///             match pane {
///                 // One destination per conversation, the open one current.
///                 NavigationPane::List => for (index, conversation) in
///                     self.conversations.iter().enumerate() {
///                     frame.navigation_item(ui, p,
///                         BoundAction::new(ChatAction::Open, conversation.id.as_str()),
///                         DestinationSpec {
///                             icon: IconId::User,
///                             label: &conversation.name,
///                             description: "Open the conversation",
///                             selected: self.open == Some(index),
///                             badge: None,
///                         });
///                 },
///                 NavigationPane::Content => drop(ui.label("The messages")),
///                 NavigationPane::Detail => drop(ui.label("The details")),
///                 NavigationPane::Empty => drop(ui.label("Choose a conversation")),
///             }
///         });
///     }
///
///     fn apply(&mut self, _: (), _: &mut ()) -> Result<bool, String> {
///         Ok(false)
///     }
///
///     // Back closes the deepest open level; the toggle turns the detail over.
///     fn perform(&mut self, target: ActionTarget<ChatAction>, _: &mut ()) -> Result<bool, String> {
///         match target.action {
///             ChatAction::Back if self.details => self.details = false,
///             ChatAction::Back => self.open = None,
///             ChatAction::Details => self.details = !self.details,
///             ChatAction::Open => return Err("Choose a conversation".into()),
///         }
///         Ok(true)
///     }
///
///     // Rows and agents deliver the conversation's ID.
///     fn perform_with(&mut self, target: ActionTarget<ChatAction>, argument: ActionArgument,
///         state: &mut ()) -> Result<bool, String> {
///         let ActionArgument::Text(id) = argument else {
///             return self.perform(target, state);
///         };
///         self.open = Some(self.conversations.iter().position(|c| c.id == id)
///             .ok_or("No conversation has this ID")?);
///         Ok(true)
///     }
///
///     fn report_error(&mut self, _: String) {}
/// }
/// ```
#[derive(Clone, Debug)]
pub struct NavigationSpec<'a, A> {
    /// The list pane's title, such as "Conversations".
    pub list_title: &'a str,
    /// The item open from the list, if any.
    pub content: Option<NavigationContent<'a, A>>,
    /// Back: the action that closes the deepest open level, the detail when
    /// it is shown and the item otherwise. The recipe shows it, as an
    /// `ArrowLeft` icon named by its label, wherever the level it closes
    /// covers the one beneath; agents can perform it in any layout.
    pub back: A,
}

impl<A> NavigationSpec<'_, A> {
    /// The deepest open level: the detail, the content or the list.
    pub fn level(&self) -> NavigationPane {
        match &self.content {
            None => NavigationPane::List,
            Some(content) if content.detail.as_ref().is_some_and(|detail| detail.shown) => {
                NavigationPane::Detail
            }
            Some(_) => NavigationPane::Content,
        }
    }
}

/// What a navigation recipe presents beyond its [`NavigationSpec`]: an
/// avatar, a one-line subtitle and controls in the open item's header, and
/// which panes' bodies are inset from the pane's edges. Pass it to
/// [`PresentationContext::navigation_with`] or
/// [`Frame::navigation_with`](crate::Frame::navigation_with).
///
/// The content's header then reads Back (where the layout shows it), the
/// avatar, the title over the subtitle, the controls in the order given
/// and the detail toggle last. Controls keep their whole size and the title
/// elides in the room they leave. When they would leave the title less than
/// its least width (its whole text, or three minimum hit targets), the
/// controls and the toggle take a second line of the header, end-aligned, so
/// no control is ever dropped. At a phone's 390 points, Back, a header
/// avatar, a title and three icon controls besides the toggle fit on one
/// line at the reference text size; give further actions their place in the
/// detail.
///
/// ```
/// use stobie_ui_egui::{
///     ActionKey, AvatarSize, AvatarSpec, BoundAction, HeaderControl, IconId,
///     NavigationOptions,
/// };
///
/// // A is the application's own ActionKey enum; mute and edit are its actions.
/// fn header<'a, A: ActionKey>(name: &'a str, status: &'a str, id: &str, muted: bool, mute: A,
///     edit: A) -> NavigationOptions<'a, A> {
///     NavigationOptions::new()
///         .with_avatar(AvatarSpec::person(name, AvatarSize::Header))
///         .with_subtitle(status)
///         .with_control(HeaderControl::icon(BoundAction::new(mute, id), IconId::Notifications)
///             .pressed(muted))
///         .with_control(HeaderControl::icon(edit, IconId::Edit))
/// }
/// ```
#[derive(Clone, Debug)]
pub struct NavigationOptions<'a, A: ActionKey> {
    avatar: Option<AvatarSpec<'a>>,
    subtitle: Option<&'a str>,
    controls: Vec<HeaderControl<A>>,
    insets: Vec<(NavigationPane, bool)>,
}

impl<A: ActionKey> Default for NavigationOptions<'_, A> {
    fn default() -> Self {
        Self {
            avatar: None,
            subtitle: None,
            controls: Vec::new(),
            insets: Vec::new(),
        }
    }
}

impl<'a, A: ActionKey> NavigationOptions<'a, A> {
    /// No avatar, subtitle or controls, and the default insets.
    pub fn new() -> Self {
        Self::default()
    }

    /// Show `avatar` after Back in the content's header, normally at
    /// [`AvatarSize::Header`](crate::AvatarSize::Header). It is decorative:
    /// the title names the item.
    pub fn with_avatar(mut self, avatar: AvatarSpec<'a>) -> Self {
        self.avatar = Some(avatar);
        self
    }

    /// Show `subtitle`, such as a status ("Online", "Typing…"), on one line
    /// under the content's title, eliding with its full text kept.
    pub fn with_subtitle(mut self, subtitle: &'a str) -> Self {
        self.subtitle = Some(subtitle);
        self
    }

    /// Add a control to the content's header, after those added before and
    /// before the detail toggle. It belongs to the content's pane region.
    pub fn with_control(mut self, control: HeaderControl<A>) -> Self {
        self.controls.push(control);
        self
    }

    /// Whether `pane`'s body is inset from the pane's edges by as much as its
    /// header's controls, overriding the default (see [`Self::body_inset`]).
    pub fn with_body_inset(mut self, pane: NavigationPane, inset: bool) -> Self {
        self.insets.retain(|(other, _)| *other != pane);
        self.insets.push((pane, inset));
        self
    }

    /// Whether `pane`'s body is inset horizontally by the header's inset
    /// (`spacing.inline`). By default the content, the detail and the empty
    /// prompt are, so text and bubbles align with the header's title and
    /// keep off the screen's and the pane's edges; the list is not, so its
    /// rows' selection and hover fill its width.
    pub fn body_inset(&self, pane: NavigationPane) -> bool {
        self.insets
            .iter()
            .find(|(other, _)| *other == pane)
            .map_or(pane != NavigationPane::List, |(_, inset)| *inset)
    }
}

/// A navigation header's control as presented: its resolved target and its
/// response.
#[derive(Clone, Debug)]
pub struct NavigationControlResponse<A: ActionKey> {
    pub target: ActionTarget<A>,
    pub response: Response,
}

/// What a navigation recipe presented in one pass.
pub struct NavigationResponse<A: ActionKey> {
    /// The layout for the width the recipe had.
    pub layout: NavigationLayout,
    /// Each pane presented, from the start of the line, with its rectangle
    /// and the observations of what its closure presented.
    pub panes: Vec<NavigationPaneResponse>,
    /// Back, when this layout shows it.
    pub back: Option<NavigationControlResponse<A>>,
    /// The detail toggle, when the content shows it.
    pub toggle: Option<NavigationControlResponse<A>>,
}

/// One pane a navigation recipe presented.
pub struct NavigationPaneResponse {
    pub pane: NavigationPane,
    pub rect: Rect,
    /// The pane's region node, absent for [`NavigationPane::Empty`].
    pub id: Option<SemanticUiId>,
    /// What the closure presented in the pane; it is registered already.
    pub observations: PresentationObservations,
}

/// A pane to present and the controls its header carries.
struct PanePlan<'a> {
    pane: NavigationPane,
    rect: Rect,
    title: Option<&'a str>,
    back: bool,
    toggle: bool,
}

/// Work out which panes a layout shows for the open level, and where.
fn plan<'a, A>(
    spec: &NavigationSpec<'a, A>,
    layout: NavigationLayout,
    rect: Rect,
) -> Vec<PanePlan<'a>> {
    let level = spec.level();
    let content = spec.content.as_ref();
    let detail = content.and_then(|content| content.detail.as_ref());
    let pane = |pane, rect, back: bool| {
        let (title, toggle) = match pane {
            NavigationPane::List => (Some(spec.list_title), false),
            NavigationPane::Content => (content.map(|content| content.title), detail.is_some()),
            NavigationPane::Detail => (detail.map(|detail| detail.title), false),
            NavigationPane::Empty => (None, false),
        };
        PanePlan {
            pane,
            rect,
            title,
            back,
            toggle,
        }
    };
    let split = |rect: Rect, width: f32| {
        let cut = (rect.left() + width).min(rect.right());
        (
            Rect::from_min_max(rect.min, pos2(cut, rect.bottom())),
            Rect::from_min_max(pos2(cut, rect.top()), rect.max),
        )
    };
    match layout {
        NavigationLayout::Stack => vec![pane(level, rect, level != NavigationPane::List)],
        NavigationLayout::Split => {
            let (list, rest) = split(rect, NavigationLayout::LIST_WIDTH);
            let main = match level {
                NavigationPane::List => pane(NavigationPane::Empty, rest, false),
                NavigationPane::Content => pane(NavigationPane::Content, rest, false),
                _ => pane(NavigationPane::Detail, rest, true),
            };
            vec![pane(NavigationPane::List, list, false), main]
        }
        NavigationLayout::Columns => {
            let (list, rest) = split(rect, NavigationLayout::LIST_WIDTH);
            let mut panes = vec![pane(NavigationPane::List, list, false)];
            match level {
                NavigationPane::List => panes.push(pane(NavigationPane::Empty, rest, false)),
                NavigationPane::Content => panes.push(pane(NavigationPane::Content, rest, false)),
                _ => {
                    let (main, side) = split(rest, rest.width() - NavigationLayout::DETAIL_WIDTH);
                    panes.push(pane(NavigationPane::Content, main, false));
                    panes.push(pane(NavigationPane::Detail, side, false));
                }
            }
            panes
        }
    }
}

/// What had keyboard focus at each level the reader went past, kept so
/// Back can return focus to it.
#[derive(Clone, Default)]
struct FocusTrail {
    depth: Option<usize>,
    /// The focus each shallower level had when the reader went deeper.
    saved: Vec<Option<FocusReturn>>,
    /// Back as last presented.
    back: Option<egui::Id>,
}

fn depth(level: NavigationPane) -> usize {
    match level {
        NavigationPane::List | NavigationPane::Empty => 0,
        NavigationPane::Content => 1,
        NavigationPane::Detail => 2,
    }
}

/// Keep keyboard focus with the reader as levels open and close: opening a
/// level that shows Back moves focus that was somewhere to Back, because
/// what had it may be covered; going back returns focus to what had it at
/// that level, such as the row that opened the item, when focus was on
/// Back, nowhere, or on a control of the level that closed. When that
/// control has gone, as a row that left the list has, focus moves to the
/// control of the level's pane nearest where it was.
fn follow_focus<A: ActionKey>(
    ui: &Ui,
    scope: &PresentationScope,
    level: NavigationPane,
    rect: Rect,
    response: &NavigationResponse<A>,
) {
    let key = scope.instance("focus").egui_id();
    let mut trail = ui
        .data(|data| data.get_temp::<FocusTrail>(key))
        .unwrap_or_default();
    let now = depth(level);
    let context = ui.ctx();
    let focused = ui.memory(|memory| memory.focused());
    let back = response.back.as_ref().map(|back| back.response.id);
    match trail.depth {
        Some(previous) if now > previous => {
            trail.saved.truncate(previous);
            let saved = focused.and_then(|focused| FocusReturn::remember(context, focused));
            trail.saved.resize(now, saved);
            if let (Some(_), Some(back)) = (focused, back) {
                ui.memory_mut(|memory| memory.request_focus(back));
            }
        }
        Some(previous) if now < previous => {
            let restore = trail.saved.get(now).copied().flatten();
            trail.saved.truncate(now);
            // A control of the level that closed, such as one that hid its
            // item and went back, has gone with it.
            let closed = focused.is_some_and(|focused| {
                !focus_return::presented(context, focused)
                    && context
                        .read_response(focused)
                        .is_some_and(|previous| rect.contains(previous.rect.center()))
            });
            if let Some(restore) = restore
                && (focused.is_none() || focused == trail.back || closed)
            {
                let pane = response
                    .panes
                    .iter()
                    .find(|pane| pane.pane != NavigationPane::Empty && depth(pane.pane) == now)
                    .map_or(rect, |pane| pane.rect);
                // Nothing in the pane taking focus leaves it nowhere.
                if let Some(target) = restore.target(context, pane) {
                    ui.memory_mut(|memory| memory.request_focus(target));
                }
            }
        }
        _ => {}
    }
    trail.depth = Some(now);
    trail.back = back;
    ui.data_mut(|data| data.insert_temp(key, trail));
}

impl PresentationContext {
    /// Present a list, the item open from it and that item's detail across
    /// the width `ui` has: stacked with Back below
    /// [`NavigationLayout::SPLIT_FROM`], the list beside the content from
    /// it, and all three side by side from
    /// [`NavigationLayout::COLUMNS_FROM`] (see [`NavigationLayout`]).
    /// Hosted applications use [`Frame::navigation`](crate::Frame::navigation),
    /// which also triggers Back and the detail toggle.
    ///
    /// - `render` presents each pane's body below its header, in a child
    ///   context whose nodes are the pane's and which publishes only what
    ///   is visible, so a body may scroll. It is called once per pane shown,
    ///   with the pane: present the destinations for
    ///   [`NavigationPane::List`], marking the open one `selected`.
    /// - Each pane but [`NavigationPane::Empty`] is a region named by its
    ///   title, with a header of the application bar's height holding Back
    ///   where the layout needs it, the title as a heading and, on the
    ///   content, the detail toggle. The content's region carries
    ///   `spec.content.reference`, so agents read which item is open.
    /// - Which pane shows follows only the application's state, so a resize
    ///   across a breakpoint keeps what is open, and each pane keeps its
    ///   egui identity across layouts, so its scroll position survives too.
    /// - Keyboard focus follows the reader: opening a level that shows
    ///   Back moves focus to Back, and Back returns focus to what had it at
    ///   the level it returns to, such as the row that opened the item, or,
    ///   when that has gone, to the control of the level's pane nearest
    ///   where it was.
    /// - The recipe pushes no browser history: the in-app Back is the way
    ///   back, and the browser's own Back and the platform's back gesture
    ///   leave the application as they do for any page.
    /// - `availability` gives Back's and the toggle's availability; a
    ///   hidden control is not presented.
    /// - The content's, the detail's and the empty prompt's bodies are inset
    ///   horizontally by as much as the header's controls, the list's is
    ///   not (see [`NavigationOptions::body_inset`]).
    ///
    /// [`Self::navigation_with`] adds an avatar, a subtitle and controls to
    /// the content's header, and chooses the insets.
    pub fn navigation<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: &NavigationSpec<'_, A>,
        availability: impl Fn(ActionTarget<A>, &ActionArgument) -> Availability,
        render: impl FnMut(&mut Ui, &mut PresentationContext, NavigationPane),
    ) -> NavigationResponse<A> {
        let options = NavigationOptions::new();
        self.navigation_with(ui, key, spec, &options, availability, render)
            .0
    }

    /// [`Self::navigation`] with `options`: an avatar, a subtitle and
    /// controls in the open item's header, and which bodies are inset. It
    /// returns, besides the navigation's response, each header control
    /// shown, in the order given, for the caller to route;
    /// [`Frame::navigation_with`](crate::Frame::navigation_with) triggers
    /// them. `availability` gives the controls' availability too.
    pub fn navigation_with<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: &NavigationSpec<'_, A>,
        options: &NavigationOptions<'_, A>,
        availability: impl Fn(ActionTarget<A>, &ActionArgument) -> Availability,
        mut render: impl FnMut(&mut Ui, &mut PresentationContext, NavigationPane),
    ) -> (NavigationResponse<A>, Vec<HeaderControlResponse<A>>) {
        self.check_pass(ui);
        let scope = self.scope.child(key);
        let rect = ui.available_rect_before_wrap();
        let layout = NavigationLayout::for_width(rect.width());
        let tokens = self.tokens;
        let back_target = spec.back.into_target(self.pane);
        let toggle = spec
            .content
            .as_ref()
            .and_then(|content| content.detail.as_ref())
            .map(|detail| (detail.toggle.into_target(self.pane), detail));
        let mut response = NavigationResponse {
            layout,
            panes: Vec::new(),
            back: None,
            toggle: None,
        };
        let mut controls = Vec::new();
        let plans = plan(spec, layout, rect);
        for (index, plan) in plans.iter().enumerate() {
            // Panes beside one another share a subtle edge.
            let surface = match plan.pane {
                NavigationPane::List | NavigationPane::Detail
                    if layout != NavigationLayout::Stack =>
                {
                    tokens.colours.surface_panel
                }
                _ => tokens.colours.surface_canvas,
            };
            ui.painter().rect_filled(plan.rect, 0.0, surface);
            if index > 0 {
                ui.painter().vline(
                    plan.rect.left() + 0.5,
                    plan.rect.y_range(),
                    Stroke::new(1.0, tokens.colours.border_subtle),
                );
            }
            let id = plan
                .title
                .map(|_| scope.instance(("pane", plan.pane.key())).semantic_id());
            let reference = match plan.pane {
                NavigationPane::Content => spec
                    .content
                    .as_ref()
                    .and_then(|content| content.reference.clone()),
                _ => None,
            };
            let observations = ui
                .scope_builder(
                    UiBuilder::new()
                        .id_salt(scope.instance(("ui", plan.pane.key())).egui_id())
                        .max_rect(plan.rect)
                        .layout(Layout::top_down(Align::Min)),
                    |ui| {
                        ui.set_clip_rect(ui.clip_rect().intersect(plan.rect));
                        if let (Some(id), Some(title)) = (&id, plan.title) {
                            ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
                                node.set_role(egui::accesskit::Role::Region);
                                node.set_label(title);
                                node.set_author_id(id.0.clone());
                                node.set_bounds(egui::accesskit::Rect {
                                    x0: f64::from(plan.rect.min.x),
                                    y0: f64::from(plan.rect.min.y),
                                    x1: f64::from(plan.rect.max.x),
                                    y1: f64::from(plan.rect.max.y),
                                });
                            });
                        }
                        // Pane bodies scroll: publish only what each shows.
                        crate::observe_visible_only(ui, |ui| {
                            let mut child = PresentationContext::new(
                                ui,
                                self.tokens,
                                self.font_scale,
                                scope.child(plan.pane.key()),
                                id.clone().unwrap_or_else(|| self.parent.clone()),
                            );
                            child.pane = self.pane;
                            child.domain_reference =
                                reference.clone().or_else(|| self.domain_reference.clone());
                            let mut body = plan.rect;
                            if let Some(title) = plan.title {
                                let header = self.navigation_header(
                                    ui,
                                    &mut child,
                                    scope,
                                    plan,
                                    title,
                                    (plan.back).then_some(back_target),
                                    toggle.filter(|_| plan.toggle),
                                    (plan.pane == NavigationPane::Content).then_some(options),
                                    &availability,
                                );
                                body.min.y = header.bar.bottom();
                                if header.back.is_some() {
                                    response.back = header.back;
                                }
                                if header.toggle.is_some() {
                                    response.toggle = header.toggle;
                                }
                                controls.extend(header.controls);
                            }
                            // Text and controls keep off the pane's edges,
                            // aligned with the header's.
                            if options.body_inset(plan.pane) {
                                let inset = tokens.spacing.inline.0.min(body.width() * 0.5);
                                body.min.x += inset;
                                body.max.x -= inset;
                            }
                            ui.scope_builder(
                                UiBuilder::new()
                                    .id_salt("body")
                                    .max_rect(body)
                                    .layout(Layout::top_down(Align::Min)),
                                |ui| {
                                    ui.set_clip_rect(ui.clip_rect().intersect(body));
                                    render(ui, &mut child, plan.pane);
                                },
                            );
                            child.finish(ui)
                        })
                    },
                )
                .inner;
            if let (Some(id), Some(title)) = (&id, plan.title) {
                let mut node = UiNode::container(
                    id.clone(),
                    Some(self.parent.clone()),
                    UiRole::Pane,
                    plan.rect.into(),
                );
                node.name = title.to_owned();
                node.pane = self.pane;
                node.domain_reference = reference.or_else(|| self.domain_reference.clone());
                self.push_node(ui, node);
            }
            response.panes.push(NavigationPaneResponse {
                pane: plan.pane,
                rect: plan.rect,
                id,
                observations,
            });
        }
        follow_focus(ui, &scope, spec.level(), rect, &response);
        ui.advance_cursor_after_rect(rect);
        (response, controls)
    }

    /// A pane's header: Back, the title and the detail toggle, on the
    /// pane's own context, and for the content the options' avatar,
    /// subtitle and controls.
    #[allow(clippy::too_many_arguments)]
    fn navigation_header<A: ActionKey>(
        &self,
        ui: &mut Ui,
        child: &mut PresentationContext,
        scope: PresentationScope,
        plan: &PanePlan<'_>,
        title: &str,
        back: Option<ActionTarget<A>>,
        toggle: Option<(ActionTarget<A>, &NavigationDetail<'_, A>)>,
        options: Option<&NavigationOptions<'_, A>>,
        availability: &impl Fn(ActionTarget<A>, &ActionArgument) -> Availability,
    ) -> Header<A> {
        let tokens = self.tokens;
        let font_scale = self.font_scale;
        // Back and the details toggle are icon-only: they take the touch
        // target, and the header's line grows to hold it.
        let hit = icon_action_side(&tokens, font_scale);
        let heading = TextRole::SectionHeading
            .style(&tokens, font_scale)
            .line_height
            + tokens.spacing.block.0;
        let subtitle = options.and_then(|options| options.subtitle);
        let subtitle_height = subtitle.map_or(0.0, |_| {
            TextRole::Secondary.style(&tokens, font_scale).line_height
        });
        let avatar = options.and_then(|options| options.avatar);
        let avatar_side = avatar.map(|avatar| crate::avatar_size(&tokens, avatar.size, font_scale));
        let title_height = heading + subtitle_height;
        let line = tokens.geometry.application_bar_height.0.max(
            hit.max(title_height).max(avatar_side.unwrap_or(0.0)) + tokens.spacing.unit.0 * 2.0,
        );
        let pad = tokens.spacing.inline.0;
        let gap = tokens.spacing.unit.0;
        let icon_width = |label: &str, icon: IconId| {
            let content = ActionButtonContent::IconOnly(icon);
            let size = action_button_size(ui, label, content, &tokens, font_scale);
            size.intrinsic.max(size.minimum_width)
        };

        // What shows: Back and the avatar lead; the controls, then the
        // toggle, end the line.
        let back = back
            .map(|target| (target, availability(target, &ActionArgument::None)))
            .filter(|(_, availability)| availability.visible());
        let toggle = toggle
            .map(|(target, detail)| (target, detail, availability(target, &ActionArgument::None)))
            .filter(|(_, _, availability)| availability.visible());
        let extra = options.map_or_else(Vec::new, |options| {
            child.resolve_controls(ui, &options.controls, availability)
        });
        let mut leading = Vec::new();
        if let Some((target, _)) = &back {
            let width = icon_width(target.action.specification().label, IconId::ArrowLeft);
            leading.push(vec2(width, hit));
        }
        if let Some(side) = avatar_side {
            leading.push(vec2(side, side));
        }
        let mut trailing: Vec<f32> = extra.iter().map(|control| control.width).collect();
        if let Some((target, detail, _)) = &toggle {
            trailing.push(icon_width(target.action.specification().label, detail.icon));
        }
        let metrics = RowMetrics {
            line,
            hit,
            title: title_height,
            gap,
            min_title: minimum_title_width(ui, title, &tokens, font_scale),
        };
        let row = plan_row(
            plan.rect.left() + pad,
            plan.rect.right() - pad,
            plan.rect.top(),
            &metrics,
            &leading,
            &trailing,
        );
        // A second line of controls keeps the first line's padding below.
        let height = row.height + if row.wrapped { gap } else { 0.0 };
        let bar = Rect::from_min_size(plan.rect.min, vec2(plan.rect.width(), height));
        ui.painter().hline(
            bar.x_range(),
            bar.bottom() - 0.5,
            Stroke::new(1.0, tokens.colours.border_subtle),
        );
        let header_scope = scope.child(("header", plan.pane.key()));
        let present = |child: &mut PresentationContext,
                       ui: &mut Ui,
                       rect: Rect,
                       name: &'static str,
                       spec: ActionButtonSpec<A>,
                       icon: IconId| {
            let identity = header_scope.instance(name).action_identity();
            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                child.present_action(
                    ui,
                    &identity,
                    spec,
                    ActionButtonContent::IconOnly(icon),
                    None,
                    None,
                )
            })
            .inner
        };
        let mut header = Header {
            bar,
            back: None,
            toggle: None,
            controls: Vec::new(),
        };
        let mut leading_rects = row.leading.iter();
        if let Some((target, availability)) = back {
            let rect = *leading_rects.next().expect("Back has its place");
            let response = present(
                child,
                ui,
                rect,
                "back",
                ActionButtonSpec {
                    target,
                    availability,
                    state: ActionButtonState::Momentary,
                    emphasis: ActionEmphasis::QuietBorderless,
                    compact: false,
                },
                IconId::ArrowLeft,
            );
            header.back = Some(NavigationControlResponse { target, response });
        }
        if let (Some(avatar), Some(rect)) = (avatar, leading_rects.next()) {
            crate::paint_avatar(ui.painter(), &avatar, *rect, &tokens);
        }
        let mut trailing_rects = row.trailing.iter();
        for control in &extra {
            let rect = *trailing_rects.next().expect("each control has its place");
            header
                .controls
                .push(child.present_header_control(ui, header_scope, rect, control));
        }
        if let Some((target, detail, availability)) = toggle {
            let rect = *trailing_rects.next().expect("the toggle has its place");
            let response = present(
                child,
                ui,
                rect,
                "toggle",
                ActionButtonSpec {
                    target,
                    availability,
                    state: ActionButtonState::Toggle {
                        pressed: detail.shown,
                    },
                    emphasis: ActionEmphasis::QuietBorderless,
                    compact: false,
                },
                detail.icon,
            );
            header.toggle = Some(NavigationControlResponse { target, response });
        }
        // The title takes the room between the controls, centred on the
        // first line, over the subtitle.
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(row.title)
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                // The subtitle sits directly under the heading, whose
                // height includes its own spacing.
                ui.spacing_mut().item_spacing.y = 0.0;
                child.heading(ui, ("title", plan.pane.key()), title);
                if let Some(subtitle) = subtitle {
                    child.content(
                        ui,
                        ("subtitle", plan.pane.key()),
                        subtitle,
                        ContentTextSpec {
                            role: TextRole::Secondary,
                            overflow: TextOverflow::Ellipsis,
                            max_lines: 1,
                            interaction: TextInteraction::Selectable,
                        },
                    );
                }
            },
        );
        header
    }
}

struct Header<A: ActionKey> {
    bar: Rect,
    back: Option<NavigationControlResponse<A>>,
    toggle: Option<NavigationControlResponse<A>>,
    controls: Vec<HeaderControlResponse<A>>,
}
