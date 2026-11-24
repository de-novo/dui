//! Search the active archive using the same labels as its map cards.
use super::{DuiDesktopView, route_short_name};
use gpui_kit::{
    App, AppContext, Context, Focusable, IntoElement, ParentElement, Styled, Task, Window,
    component::{
        IndexPath, WindowExt,
        list::{List, ListDelegate, ListEvent, ListItem, ListState},
    },
    div, px,
};
use std::cell::Cell;

struct Routes {
    paths: Vec<String>,
    matches: Vec<usize>,
    empty: &'static str,
}

impl ListDelegate for Routes {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.matches.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        let term = query.trim().to_lowercase();
        self.matches = self
            .paths
            .iter()
            .enumerate()
            .filter_map(|(index, path)| {
                (path.to_lowercase().contains(&term)
                    || route_short_name(path).to_lowercase().contains(&term))
                .then_some(index)
            })
            .collect();
        // The list's row cache still describes the previous query until layout.
        // Reset selection from the new matches, including when recovering from zero rows.
        cx.defer_in(window, |list, window, cx| {
            let first = (!list.delegate().matches.is_empty()).then(IndexPath::default);
            list.set_selected_index(first, window, cx);
            cx.notify();
        });
        Task::ready(())
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let index = *self.matches.get(ix.row)?;
        Some(ListItem::new(("screen-result", index)).child(self.paths[index].clone()))
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
            .p_3()
            .child(gpui_kit::component::label::Label::new(self.empty).id("screen-search-empty"))
    }

    fn set_selected_index(
        &mut self,
        _: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
    }
}

impl DuiDesktopView {
    pub(super) fn open_screen_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.active_run() else { return };
        let strings = super::i18n::strings(self.lang);
        let title = strings.find_screen;
        let routes = Routes {
            paths: run.screens.iter().map(|s| s.path.clone()).collect(),
            matches: (0..run.screens.len()).collect(),
            empty: strings.no_matching_screens,
        };
        let list = cx.new(|cx| {
            let first = (!routes.paths.is_empty()).then(IndexPath::default);
            let mut list = ListState::new(routes, window, cx).searchable(true);
            list.set_selected_index(first, window, cx);
            list
        });
        let subscription = cx.subscribe_in(&list, window, |this, list, event, window, cx| {
            if let ListEvent::Confirm(row) = event
                && let Some(&index) = list.read(cx).delegate().matches.get(row.row)
            {
                this.open_map_screen(index, cx);
                window.close_dialog(cx);
            }
        });
        self.search_trigger.focus(window, cx);
        let initial_focus = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            // Keep the subscription alive for this dialog's lifetime.
            let _ = &subscription;
            if initial_focus.replace(false) {
                list.focus_handle(cx).focus(window, cx);
            }
            dialog.title(title).child(
                div()
                    .h(px(290.))
                    .child(List::new(&list).search_placeholder(title)),
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::{DuiDesktopView, LaunchOptions, UiLang, ViewMode, route_short_name};
    use gpui_kit::{AppContext, TestAppContext, WindowOptions, test::TestWindowExt};
    use std::{env, path::PathBuf};

    #[gpui_kit::test]
    #[ignore = "run with a captured archive via npm run check:desktop-search"]
    fn search_captured_screens(cx: &mut TestAppContext) {
        let dui_home = PathBuf::from(env::var_os("DUI_DESKTOP_SEARCH_HOME").expect("test archive"));
        let (handle, view) = cx
            .update(|cx| {
                gpui_kit::init(cx);
                cx.set_reduce_motion(true);
                let opts = LaunchOptions {
                    dui_home,
                    project: None,
                    run_id: None,
                    screen_path: None,
                    view: ViewMode::Overview,
                    canvas_diff: false,
                    dark: true,
                    lang: UiLang::En,
                    viewport_width: 1440,
                    viewport_height: 920,
                };
                gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                    cx.new(|cx| DuiDesktopView::new(opts, window, cx))
                })
            })
            .expect("open desktop map");
        let paths = cx.read_entity(&view, |v, _| {
            v.active_run()
                .expect("active run")
                .screens
                .iter()
                .map(|s| s.path.clone())
                .collect::<Vec<_>>()
        });
        let target = paths
            .iter()
            .find(|p| p.contains("#overlay:"))
            .expect("captured overlay");
        let name = route_short_name(target);
        let term = name.trim_start_matches("◫ ").to_uppercase();
        assert!(
            !target.to_uppercase().contains(&term),
            "test needs a distinct display name"
        );
        macro_rules! frame {
            (|$w:ident, $cx:ident| $body:expr) => {{
                cx.update_window(handle, |_, $w, $cx| {
                    $w.render_frame($cx);
                    $body
                })
                .expect("desktop interaction");
                cx.run_until_parked();
            }};
        }
        frame!(|w, cx| w.click("find-screen", cx));
        frame!(|w, cx| {
            assert!(w.find("dialog").visible());
            w.input(&format!("{}-unmatched", paths.join("|")), cx);
        });
        frame!(|w, cx| {
            assert!(w.find("screen-search-empty").visible());
            w.press("enter", cx);
        });
        frame!(|w, cx| {
            assert!(w.find("dialog").visible());
            w.press("escape", cx);
        });
        frame!(|w, cx| {
            assert!(w.try_find("dialog").is_none());
            assert!(view.read(cx).search_trigger.is_focused(w));
            w.press("ctrl-k", cx);
        });
        frame!(|w, cx| w.input(&term, cx));
        frame!(|w, cx| w.press("enter", cx));
        frame!(|w, cx| {
            assert_eq!(view.read(cx).view_mode, ViewMode::Detail);
            assert_eq!(
                view.read(cx).active_screen().expect("opened screen").path,
                *target
            );
            assert!(w.try_find("dialog").is_none());
            w.press("ctrl-k", cx);
        });
        frame!(|w, cx| w.input(target, cx));
        frame!(|w, cx| w.press("enter", cx));
        frame!(|w, cx| {
            assert!(w.try_find("dialog").is_none());
            assert_eq!(
                view.read(cx).active_screen().expect("route match").path,
                *target
            );
            w.press("ctrl-k", cx);
        });
        frame!(|w, cx| w.press("down", cx));
        frame!(|w, cx| w.press("enter", cx));
        assert_eq!(
            cx.read_entity(&view, |v, _| v
                .active_screen()
                .expect("next screen")
                .path
                .clone()),
            paths[1]
        );
    }
}
