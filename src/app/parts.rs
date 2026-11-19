//! The window's parts around the editor as cached views (issue 01 of
//! `.scratch/performance`): the title bar, the sidebars, the tab bar and the
//! status bar.
//!
//! GPUI renders `NotekiApp` from its root at every frame the editor asks for
//! (a key, the caret), as the editor is its child. Each part here is a view
//! of its own, drawn with GPUI's view caching: a frame where only the editor
//! changed reuses their last layout and paint. A part is drawn again when
//! `NotekiApp` notifies (what it shows changed: after a pause in typing, a
//! click, a Vault event) or when one of its own elements does (a hover).

use gpui_kit::{
    div, px, AppContext as _, Context, Entity, IntoElement, Render, Styled as _, Subscription,
    WeakEntity, Window,
};

use super::NotekiApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PartKind {
    TitleBar,
    LeftSidebar,
    RightSidebar,
    TabBar,
    StatusBar,
}

pub(super) struct Part {
    app: WeakEntity<NotekiApp>,
    kind: PartKind,
    _observe: Subscription,
}

impl Part {
    pub(super) fn new(
        app: &Entity<NotekiApp>,
        kind: PartKind,
        cx: &mut gpui_kit::App,
    ) -> Entity<Self> {
        cx.new(|cx| Self {
            app: app.downgrade(),
            kind,
            _observe: cx.observe(app, |_, _, cx| cx.notify()),
        })
    }

    /// `part` as a cached view, at its size: a cached view is not measured
    /// from what it draws.
    pub(super) fn cached(part: &Entity<Self>, cx: &gpui_kit::App) -> impl IntoElement {
        let size = div().flex_shrink_0();
        let mut size = match part.read(cx).kind {
            PartKind::TitleBar => size.w_full().h(gpui_kit::component::TITLE_BAR_HEIGHT),
            PartKind::TabBar => size.w_full().h(super::render::TAB_BAR_HEIGHT),
            PartKind::StatusBar => size.w_full().h(super::render::STATUS_BAR_HEIGHT),
            PartKind::LeftSidebar | PartKind::RightSidebar => size.size_full().min_w(px(0.)),
        };
        part.clone().cached(std::mem::take(size.style()))
    }
}

impl Render for Part {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = self.kind;
        self.app
            .update(cx, |app, cx| match kind {
                PartKind::TitleBar => app.render_title_bar(cx).into_any_element(),
                PartKind::LeftSidebar => app.render_left_sidebar(cx).into_any_element(),
                PartKind::RightSidebar => app.render_right_sidebar(cx).into_any_element(),
                PartKind::TabBar => app.render_tab_bar(cx).into_any_element(),
                PartKind::StatusBar => app.render_status_bar(cx).into_any_element(),
            })
            .unwrap_or_else(|_| div().into_any_element())
    }
}
