mod i18n;
mod layout;
mod live_client;
mod store;
mod tokens;

use gpui_kit::{
    AppContext, Bounds, Context, Entity, FocusHandle, Focusable as _, HeadlessAppContext, Hsla,
    ImageSource, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PathBuilder, Pixels, Point, Render, RenderImage, ScrollDelta, ScrollEvent, SharedString,
    Window, WindowBounds, WindowOptions,
    assets::Assets,
    base::StyledExt,
    canvas,
    component::{
        Theme, ThemeMode,
        button::{Button, ButtonVariants as _},
        input::{Input, InputState},
        label::Label,
    },
    div, img, point,
    prelude::*,
    px, size,
    test::{TestQueryExt, TestWindowExt},
};
use i18n::UiLang;
use layout::{MapLayout, compute_layout};
use live_client::LiveSessionState;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
    sync::Arc,
};
use store::{
    CellItem, DesktopWorkspace, DiffCellEntry, ImageCache, NewCommentAnchor, RunDesignAudit,
    RunItem, ScreenItem, compute_run_design_audit, design_severity_color_hex,
};
use tokens::Palette;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ViewMode {
    Overview,
    Detail,
    Live,
    Diff,
}

impl ViewMode {
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "detail" => Self::Detail,
            "live" => Self::Live,
            "diff" => Self::Diff,
            _ => Self::Overview,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CommentTargetChoice {
    Screen,
    Cell,
    Transition(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CanvasDiffVisualMode {
    Render,
    After,
    Before,
    Overlay,
}

struct LaunchOptions {
    dui_home: PathBuf,
    project: Option<String>,
    run_id: Option<String>,
    screen_path: Option<String>,
    live_base_url: Option<String>,
    view: ViewMode,
    canvas_diff: bool,
    dark: bool,
    lang: UiLang,
    viewport_width: u32,
    viewport_height: u32,
}

fn validate_loopback_live_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let rest = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))?;
    let host_port = rest.split('/').next().unwrap_or("");
    let host = host_port.split(':').next().unwrap_or("");
    if host == "127.0.0.1" || host == "localhost" {
        Some(trimmed.to_owned())
    } else {
        None
    }
}

struct DuiDesktopView {
    dui_home: PathBuf,
    workspace: DesktopWorkspace,
    images: ImageCache,
    selected_run: usize,
    compare_run: Option<usize>,
    selected_screen: usize,
    selected_width: u32,
    selected_cell: usize,
    neighbor_page: usize,
    full_1x_detail: bool,
    view_mode: ViewMode,
    rail_collapsed: bool,
    picker_open: bool,
    options_open: bool,
    dark_mode: bool,
    lang: UiLang,
    // 2D infinite canvas camera
    pan_x: f32,
    pan_y: f32,
    zoom: f32,
    dragging: bool,
    drag_moved: bool,
    drag_last: Point<Pixels>,
    viewport_w: f32,
    viewport_h: f32,
    // Comment composer
    focus_handle: FocusHandle,
    comment_input: Entity<InputState>,
    comment_target: CommentTargetChoice,
    error_message: Option<String>,
    // Active `dui live` session & optional loopback live surface URL (`--live-url` / `DUI_LIVE_URL`)
    live_session: Option<LiveSessionState>,
    live_base_url: Option<String>,
    // Cached diff entries for current (before, after) pair
    diff_cache_key: Option<(usize, usize)>,
    diff_entries: Vec<DiffCellEntry>,
    // 2D Canvas inline diff controls (matching `apps/canvas`)
    canvas_diff_enabled: bool,
    canvas_diff_changed_only: bool,
    canvas_diff_mode: CanvasDiffVisualMode,
    // Design-defect & Element Inspector state (matching `apps/canvas` #105 / #106)
    design_audit_run: Option<usize>,
    design_audit: RunDesignAudit,
    show_design_overlay: bool,
    selected_element_id: Option<String>,
}

impl Drop for DuiDesktopView {
    fn drop(&mut self) {
        if let Some(session) = self.live_session.take() {
            live_client::close_session(&session.session_id);
        }
    }
}

fn route_short_name(path: &str) -> String {
    let clean = path.split('?').next().unwrap_or(path);
    let parts: Vec<&str> = clean.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return path.to_owned();
    }
    let start = parts.len().saturating_sub(2);
    parts[start..].join("/")
}

fn short_run_id(id: &str) -> &str {
    id.split('-').next_back().unwrap_or(id)
}

impl DuiDesktopView {
    fn new(opts: LaunchOptions, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspace =
            store::load_workspace(&opts.dui_home).unwrap_or_else(|_| DesktopWorkspace {
                dui_home: opts.dui_home.clone(),
                db_path: opts.dui_home.join("dui.db"),
                runs: Vec::new(),
                comments: Vec::new(),
            });

        let selected_run = if let Some(ref target_run) = opts.run_id {
            workspace
                .runs
                .iter()
                .position(|r| r.id == *target_run || r.id.ends_with(target_run))
                .unwrap_or(0)
        } else if let Some(ref pid) = opts.project {
            workspace
                .runs
                .iter()
                .position(|r| r.project_id == *pid || r.project_name == *pid)
                .unwrap_or(0)
        } else {
            workspace
                .runs
                .iter()
                .position(|r| {
                    r.screens
                        .iter()
                        .flat_map(|s| s.cells.iter())
                        .any(|c| c.status_kind == "verified" && c.blob_path.is_some())
                })
                .unwrap_or(0)
        };

        let selected_screen = opts
            .screen_path
            .as_deref()
            .and_then(|target| {
                workspace
                    .runs
                    .get(selected_run)?
                    .screens
                    .iter()
                    .position(|s| s.path == target || s.path.ends_with(target))
            })
            .unwrap_or(0);

        let selected_width = workspace
            .runs
            .get(selected_run)
            .and_then(|r| r.widths().into_iter().max())
            .unwrap_or(1280);

        let compare_run = workspace.runs.get(selected_run).and_then(|active| {
            workspace
                .runs
                .iter()
                .enumerate()
                .find(|(idx, r)| *idx != selected_run && r.project_id == active.project_id)
                .map(|(idx, _)| idx)
        });

        let placeholder = i18n::strings(opts.lang).comment_placeholder;
        let focus_handle = cx.focus_handle();
        let comment_input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));

        let mut this = Self {
            dui_home: opts.dui_home,
            workspace,
            images: ImageCache::default(),
            selected_run,
            compare_run,
            selected_screen,
            selected_width,
            selected_cell: 0,
            neighbor_page: 0,
            full_1x_detail: false,
            view_mode: opts.view,
            rail_collapsed: false,
            picker_open: false,
            options_open: false,
            dark_mode: opts.dark,
            lang: opts.lang,
            pan_x: 32.0,
            pan_y: 32.0,
            zoom: 0.12,
            dragging: false,
            drag_moved: false,
            drag_last: point(px(0.0), px(0.0)),
            viewport_w: opts.viewport_width.saturating_sub(220).max(400) as f32,
            viewport_h: opts.viewport_height.max(400) as f32,
            focus_handle,
            comment_input,
            comment_target: CommentTargetChoice::Screen,
            error_message: None,
            live_session: None,
            live_base_url: opts.live_base_url,
            diff_cache_key: None,
            diff_entries: Vec::new(),
            canvas_diff_enabled: opts.canvas_diff,
            canvas_diff_changed_only: false,
            canvas_diff_mode: CanvasDiffVisualMode::Render,
            design_audit_run: None,
            design_audit: RunDesignAudit::default(),
            show_design_overlay: false,
            selected_element_id: None,
        };
        this.ensure_design_audit();
        this.sync_cell_to_width();
        this.fit_map();
        if this.view_mode == ViewMode::Diff || this.canvas_diff_enabled {
            this.ensure_diff_loaded();
        }
        this
    }

    fn sync_cell_to_width(&mut self) {
        let Some(screen) = self.active_screen() else {
            self.selected_cell = 0;
            return;
        };
        let found = screen
            .cells
            .iter()
            .position(|c| c.width_px == self.selected_width);
        let fallback_width = screen.cells.first().map(|c| c.width_px);
        if let Some(idx) = found {
            self.selected_cell = idx;
        } else {
            self.selected_cell = 0;
            if let Some(w) = fallback_width {
                self.selected_width = w;
            }
        }
    }

    fn pal(&self) -> Palette {
        tokens::palette(self.dark_mode)
    }

    fn active_run(&self) -> Option<&RunItem> {
        self.workspace.runs.get(self.selected_run)
    }

    fn active_screen(&self) -> Option<&ScreenItem> {
        self.active_run()?.screens.get(self.selected_screen)
    }

    fn active_cell(&self) -> Option<&CellItem> {
        let screen = self.active_screen()?;
        screen
            .cells
            .get(self.selected_cell)
            .or_else(|| {
                screen
                    .cells
                    .iter()
                    .find(|c| c.width_px == self.selected_width)
            })
            .or_else(|| screen.cells.first())
    }

    fn cell_for_width<'a>(&self, screen: &'a ScreenItem) -> Option<&'a CellItem> {
        screen
            .cells
            .iter()
            .find(|c| c.width_px == self.selected_width && c.blob_path.is_some())
            .or_else(|| {
                screen
                    .cells
                    .iter()
                    .find(|c| c.width_px == self.selected_width)
            })
            .or_else(|| screen.cells.iter().find(|c| c.blob_path.is_some()))
            .or_else(|| screen.cells.first())
    }

    fn current_layout(&self) -> MapLayout {
        let Some(run) = self.active_run() else {
            return compute_layout(&[], self.selected_width);
        };
        compute_layout(&run.screens, self.selected_width)
    }

    fn fit_map(&mut self) {
        let layout = self.current_layout();
        if layout.nodes.is_empty() || layout.content_width <= 0.0 || layout.content_height <= 0.0 {
            self.zoom = 0.2;
            self.pan_x = 32.0;
            self.pan_y = 32.0;
            return;
        }
        let pad_x = 56.0;
        let pad_y = 64.0;
        let avail_w = (self.viewport_w - pad_x * 2.0).max(240.0);
        let avail_h = (self.viewport_h - pad_y * 2.0).max(240.0);
        let zx = avail_w / layout.content_width;
        let zy = avail_h / layout.content_height;
        self.zoom = zx.min(zy).clamp(0.05, 1.0);
        let scaled_w = layout.content_width * self.zoom;
        let scaled_h = layout.content_height * self.zoom;
        self.pan_x = ((self.viewport_w - scaled_w) * 0.5).max(pad_x * 0.5);
        self.pan_y = ((self.viewport_h - scaled_h) * 0.5).max(pad_y * 0.5);
    }

    fn zoom_one_to_one(&mut self) {
        let layout = self.current_layout();
        self.zoom = 1.0;
        if let Some(node) = layout
            .nodes
            .iter()
            .find(|n| n.screen_idx == self.selected_screen)
            .or_else(|| layout.nodes.first())
        {
            let cx_world = node.x + layout.metrics.card_width * 0.5;
            let cy_world = node.y + node.card_height.min(900.0) * 0.5;
            self.pan_x = self.viewport_w * 0.5 - cx_world;
            self.pan_y = self.viewport_h * 0.5 - cy_world;
        }
    }

    fn adjust_zoom(&mut self, factor: f32) {
        let next = (self.zoom * factor).clamp(0.04, 3.0);
        let ratio = next / self.zoom;
        let anchor_x = self.viewport_w * 0.5;
        let anchor_y = self.viewport_h * 0.5;
        self.pan_x = anchor_x - (anchor_x - self.pan_x) * ratio;
        self.pan_y = anchor_y - (anchor_y - self.pan_y) * ratio;
        self.zoom = next;
    }

    fn select_run(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.selected_run = idx;
        self.design_audit_run = None;
        self.ensure_design_audit();
        self.selected_screen = 0;
        self.selected_cell = 0;
        self.neighbor_page = 0;
        self.full_1x_detail = false;
        self.picker_open = false;
        self.comment_target = CommentTargetChoice::Screen;
        if let Some(run) = self.workspace.runs.get(idx) {
            let widths = run.widths();
            if !widths.contains(&self.selected_width) {
                self.selected_width = widths.into_iter().max().unwrap_or(1280);
            }
            self.compare_run = self
                .workspace
                .runs
                .iter()
                .enumerate()
                .find(|(i, r)| *i != idx && r.project_id == run.project_id)
                .map(|(i, _)| i);
        }
        self.sync_cell_to_width();
        self.diff_cache_key = None;
        self.fit_map();
        if self.view_mode == ViewMode::Diff {
            self.ensure_diff_loaded();
        }
        cx.notify();
    }

    fn ensure_diff_loaded(&mut self) {
        let Some(before_idx) = self.compare_run else {
            self.diff_entries.clear();
            self.diff_cache_key = None;
            return;
        };
        let want_pixels = self.view_mode == ViewMode::Diff
            || self.canvas_diff_mode == CanvasDiffVisualMode::Overlay;
        let key = (before_idx, self.selected_run);
        if self.diff_cache_key == Some(key) {
            if want_pixels {
                store::hydrate_pixel_diffs(&mut self.diff_entries);
            }
            return;
        }
        if let (Some(before), Some(after)) = (
            self.workspace.runs.get(before_idx),
            self.workspace.runs.get(self.selected_run),
        ) {
            self.diff_entries = store::compute_run_diff(before, after, want_pixels);
            self.diff_cache_key = Some(key);
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        match store::load_workspace(&self.dui_home) {
            Ok(ws) => {
                self.workspace = ws;
                if self.selected_run >= self.workspace.runs.len() {
                    self.selected_run = 0;
                }
                if let Some(run) = self.workspace.runs.get(self.selected_run)
                    && self.selected_screen >= run.screens.len()
                {
                    self.selected_screen = 0;
                }
                self.sync_cell_to_width();
                self.design_audit_run = None;
                self.ensure_design_audit();
                self.diff_cache_key = None;
                if self.view_mode == ViewMode::Diff {
                    self.ensure_diff_loaded();
                }
                self.error_message = None;
            }
            Err(err) => self.error_message = Some(err),
        }
        cx.notify();
    }

    fn post_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw_text: SharedString = self.comment_input.read(cx).value();
        let trimmed = raw_text.trim().to_owned();
        if trimmed.is_empty() {
            return;
        }
        let (Some(run), Some(screen)) = (self.active_run(), self.active_screen()) else {
            return;
        };
        let project_id = run.project_id.clone();
        let run_id = run.id.clone();
        let screen_path = screen.path.clone();
        let anchor = match &self.comment_target {
            CommentTargetChoice::Screen => NewCommentAnchor::Screen(screen_path),
            CommentTargetChoice::Cell => {
                if let Some(cell) = self.active_cell() {
                    NewCommentAnchor::Cell {
                        path: screen_path,
                        cell_id: cell.id.clone(),
                        width_px: cell.width_px,
                    }
                } else {
                    NewCommentAnchor::Screen(screen_path)
                }
            }
            CommentTargetChoice::Transition(to_path) => NewCommentAnchor::Transition {
                from_path: screen_path,
                to_path: to_path.clone(),
            },
        };
        match store::add_comment(
            &self.dui_home,
            &project_id,
            &run_id,
            anchor,
            "reviewer",
            &trimmed,
        ) {
            Ok(_) => {
                self.comment_input.update(cx, |state, cx| {
                    state.set_value("", window, cx);
                });
                self.reload(cx);
            }
            Err(err) => {
                self.error_message = Some(err);
                cx.notify();
            }
        }
    }

    fn toggle_comment(&mut self, comment_id: String, cx: &mut Context<Self>) {
        match store::toggle_comment_status(&self.dui_home, &comment_id) {
            Ok(_) => self.reload(cx),
            Err(err) => {
                self.error_message = Some(err);
                cx.notify();
            }
        }
    }

    fn open_live_session(&mut self, cx: &mut Context<Self>) {
        if let Some(prev) = self.live_session.take() {
            live_client::close_session(&prev.session_id);
        }
        let Some(run) = self.active_run().cloned() else {
            return;
        };
        let Some(cell) = self.active_cell().cloned() else {
            return;
        };
        let (Some(project), Some(run_dir)) = (run.project_path.as_deref(), run.run_dir.as_deref())
        else {
            self.error_message = Some("Local project checkout unavailable for live session".into());
            cx.notify();
            return;
        };
        match live_client::open_session(project, run_dir, &cell.state_id, cell.width_px) {
            Ok(session) => {
                self.live_session = Some(session);
                self.view_mode = ViewMode::Live;
                self.error_message = None;
            }
            Err(err) => {
                self.error_message = Some(format!("Live session: {err}"));
            }
        }
        cx.notify();
    }

    fn close_live_session(&mut self, return_mode: ViewMode, cx: &mut Context<Self>) {
        if let Some(session) = self.live_session.take() {
            live_client::close_session(&session.session_id);
        }
        self.view_mode = return_mode;
        cx.notify();
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .comment_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
        {
            return;
        }
        let key = event.keystroke.key.as_str();
        match key {
            "escape" => {
                if self.picker_open || self.options_open {
                    self.picker_open = false;
                    self.options_open = false;
                } else if self.view_mode == ViewMode::Live {
                    self.close_live_session(ViewMode::Detail, cx);
                    return;
                } else if self.full_1x_detail {
                    self.full_1x_detail = false;
                } else if self.view_mode == ViewMode::Detail {
                    self.view_mode = ViewMode::Overview;
                }
                cx.notify();
            }
            "+" | "=" => {
                if matches!(self.view_mode, ViewMode::Overview | ViewMode::Detail) {
                    self.adjust_zoom(1.25);
                    cx.notify();
                }
            }
            "-" => {
                if matches!(self.view_mode, ViewMode::Overview | ViewMode::Detail) {
                    self.adjust_zoom(0.8);
                    cx.notify();
                }
            }
            "0" => {
                if matches!(self.view_mode, ViewMode::Overview | ViewMode::Detail) {
                    self.zoom_one_to_one();
                    cx.notify();
                }
            }
            "!" => {
                if matches!(self.view_mode, ViewMode::Overview | ViewMode::Detail) {
                    self.fit_map();
                    cx.notify();
                }
            }
            "1" if event.keystroke.modifiers.shift => {
                if matches!(self.view_mode, ViewMode::Overview | ViewMode::Detail) {
                    self.fit_map();
                    cx.notify();
                }
            }
            _ => {}
        }
    }

    /// Floating rounded glass left WorkspaceRail matching `apps/canvas/src/features/workspace/WorkspaceRail.tsx` 1:1.
    fn render_workspace_rail(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.pal();
        let tr = i18n::strings(self.lang);
        let collapsed = self.rail_collapsed;
        let rail_w = if collapsed { 52.0 } else { 204.0 };

        let wordmark = div()
            .flex()
            .items_baseline()
            .px_2()
            .pt_2()
            .pb_3()
            .child(
                Label::new(if collapsed { "d" } else { "dui" })
                    .text_xl()
                    .font_bold()
                    .text_color(p.sidebar_foreground),
            )
            .child(Label::new(".").text_xl().font_bold().text_color(p.primary));

        let workspace_btn = if let Some(run) = self.active_run() {
            let short_ver = short_run_id(&run.id);
            let sub = match run.branch.as_deref() {
                Some(b) if !b.is_empty() && b != "main" => format!("{b} · {short_ver}"),
                _ => short_ver.to_owned(),
            };
            div()
                .id("rail-workspace-switch")
                .w_full()
                .px_2()
                .py_2()
                .rounded_md()
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.picker_open = !this.picker_open;
                        this.options_open = false;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .overflow_hidden()
                        .child(Label::new("📁").text_sm().text_color(p.sidebar_muted))
                        .when(!collapsed, |el| {
                            el.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .overflow_hidden()
                                    .child(
                                        Label::new(run.project_name.clone())
                                            .text_sm()
                                            .font_semibold()
                                            .text_color(p.sidebar_foreground),
                                    )
                                    .child(Label::new(sub).text_xs().text_color(p.sidebar_muted)),
                            )
                        }),
                )
                .when(!collapsed, |el| {
                    el.child(Label::new("↕").text_xs().text_color(p.sidebar_muted))
                })
        } else {
            div()
                .id("rail-workspace-empty")
                .w_full()
                .px_2()
                .py_2()
                .rounded_md()
                .child(
                    Label::new(tr.select_workspace)
                        .text_xs()
                        .text_color(p.sidebar_muted),
                )
        };

        let lang_row = div()
            .id("rail-lang-switch")
            .w_full()
            .px_2()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.lang = this.lang.next();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Label::new("🌐").text_xs().text_color(p.sidebar_muted))
                    .when(!collapsed, |el| {
                        el.child(
                            Label::new(self.lang.display_name())
                                .text_xs()
                                .text_color(p.sidebar_muted),
                        )
                    }),
            )
            .when(!collapsed, |el| {
                el.child(Label::new("⌄").text_xs().text_color(p.sidebar_muted))
            });

        let theme_row = div()
            .id("rail-theme-switch")
            .w_full()
            .px_2()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.dark_mode = !this.dark_mode;
                    Theme::change(
                        if this.dark_mode {
                            ThemeMode::Dark
                        } else {
                            ThemeMode::Light
                        },
                        None,
                        cx,
                    );
                    cx.notify();
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Label::new(if self.dark_mode { "☾" } else { "☀" })
                            .text_xs()
                            .text_color(p.sidebar_muted),
                    )
                    .when(!collapsed, |el| {
                        el.child(
                            Label::new(if self.dark_mode {
                                tr.theme_dark
                            } else {
                                tr.theme_light
                            })
                            .text_xs()
                            .text_color(p.sidebar_muted),
                        )
                    }),
            )
            .when(!collapsed, |el| {
                el.child(Label::new("⌄").text_xs().text_color(p.sidebar_muted))
            });

        let collapse_row = div()
            .id("rail-collapse-toggle")
            .w_full()
            .px_2()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .flex()
            .items_center()
            .gap_2()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.rail_collapsed = !this.rail_collapsed;
                    cx.notify();
                }),
            )
            .child(
                Label::new(if collapsed { "▸" } else { "◧" })
                    .text_xs()
                    .text_color(p.sidebar_muted),
            )
            .when(!collapsed, |el| {
                el.child(
                    Label::new(tr.collapse_rail)
                        .text_xs()
                        .text_color(p.sidebar_muted),
                )
            });

        div()
            .id("workspace-rail")
            .w(px(rail_w))
            .h_full()
            .flex_shrink_0()
            .p_2()
            .child(
                div()
                    .size_full()
                    .p_2()
                    .rounded_xl()
                    .border_1()
                    .border_color(p.sidebar_line)
                    .bg(p.sidebar)
                    .shadow_md()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(wordmark)
                    .child(workspace_btn)
                    .child(div().flex_1())
                    .child(lang_row)
                    .child(div().h(px(16.0)))
                    .child(theme_row)
                    .child(collapse_row),
            )
    }

    fn ensure_design_audit(&mut self) {
        if self.design_audit_run == Some(self.selected_run) {
            return;
        }
        if let Some(run) = self.workspace.runs.get(self.selected_run) {
            self.design_audit = compute_run_design_audit(run);
            self.design_audit_run = Some(self.selected_run);
        } else {
            self.design_audit = RunDesignAudit::default();
            self.design_audit_run = None;
        }
        self.selected_element_id = None;
    }

    /// Floating Review & Design Audit Pill at top-center matching `apps/canvas` `.audit-summary-pill`.
    fn render_audit_pill(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.pal();
        let Some(run) = self.active_run() else {
            return div().id("audit-pill-empty");
        };
        let open_comments: usize = self
            .workspace
            .comments
            .iter()
            .filter(|c| c.project_id == run.project_id && c.status == "open")
            .count();
        let design_total: usize = self
            .design_audit
            .findings_by_screen_width
            .iter()
            .filter(|((_, w), _)| *w == self.selected_width)
            .map(|(_, list)| list.len())
            .sum();
        if open_comments == 0 && design_total == 0 {
            return div().id("audit-pill-empty");
        }
        let overlay_active = self.show_design_overlay;

        let (open_text, design_text) = match self.lang {
            UiLang::Ko => (
                format!("미해결 코멘트 {open_comments}개"),
                format!("디자인 결함 {design_total}건"),
            ),
            UiLang::ZhCn => (
                format!("{open_comments} 条待处理评论"),
                format!("{design_total} 项设计问题"),
            ),
            UiLang::En => (
                format!("{open_comments} unresolved"),
                format!(
                    "{design_total} design {}",
                    if design_total == 1 { "issue" } else { "issues" }
                ),
            ),
        };

        div()
            .id("desktop-audit-pill")
            .px_3()
            .py_1()
            .rounded_full()
            .border_1()
            .border_color(p.glass_line)
            .bg(p.glass)
            .shadow_md()
            .flex()
            .items_center()
            .gap_3()
            .when(open_comments > 0, |el| {
                el.child(
                    Label::new(open_text)
                        .text_xs()
                        .font_semibold()
                        .text_color(p.primary),
                )
            })
            .when(design_total > 0, |el| {
                let mut pill_bg = p.primary;
                if !overlay_active {
                    pill_bg.a = 0.16;
                }
                el.child(
                    div()
                        .id("desktop-design-overlay-toggle")
                        .px_2()
                        .py_0p5()
                        .rounded_full()
                        .border_1()
                        .border_color(p.primary)
                        .bg(pill_bg)
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.show_design_overlay = !this.show_design_overlay;
                                cx.notify();
                            }),
                        )
                        .child(
                            Label::new(design_text)
                                .text_xs()
                                .font_semibold()
                                .text_color(if overlay_active {
                                    p.primary_foreground
                                } else {
                                    p.primary
                                }),
                        ),
                )
            })
    }

    fn render_live_surface_pill(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.pal();
        let Some(live_url) = self.live_base_url.clone() else {
            return div().id("desktop-live-surface-empty");
        };
        let screen_path = self
            .active_screen()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| "/".to_owned());
        let label_text = match self.lang {
            UiLang::Ko => format!("● 라이브 · {live_url}"),
            UiLang::ZhCn => format!("● 实时 · {live_url}"),
            UiLang::En => format!("● Live · {live_url}"),
        };
        div()
            .id("desktop-live-surface-pill")
            .px_3()
            .py_1()
            .rounded_full()
            .border_1()
            .border_color(p.primary)
            .bg(p.glass)
            .shadow_md()
            .cursor_pointer()
            .flex()
            .items_center()
            .gap_1p5()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    let target = format!(
                        "{}{}",
                        live_url.trim_end_matches('/'),
                        if screen_path.starts_with('/') {
                            screen_path.clone()
                        } else {
                            format!("/{screen_path}")
                        }
                    );
                    let opener = if cfg!(target_os = "macos") {
                        "open"
                    } else {
                        "xdg-open"
                    };
                    let _ = std::process::Command::new(opener).arg(&target).spawn();
                    let _ = this;
                    cx.notify();
                }),
            )
            .child(
                Label::new(label_text)
                    .text_xs()
                    .font_semibold()
                    .text_color(p.primary),
            )
    }

    /// Floating inline Canvas Diff Bar next to the audit pill, matching `apps/canvas` `.canvas-diff-bar`.
    fn render_canvas_diff_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.pal();
        let Some(run) = self.active_run() else {
            return div().id("canvas-diff-bar-empty");
        };
        let other_runs: Vec<(usize, String)> = self
            .workspace
            .runs
            .iter()
            .enumerate()
            .filter(|(idx, _)| *idx != self.selected_run)
            .map(|(idx, r)| {
                let br = r.branch.as_deref().unwrap_or("—");
                (
                    idx,
                    format!("{} · {br} · {}", r.project_name, short_run_id(&r.id)),
                )
            })
            .collect();
        if other_runs.is_empty() {
            return div().id("canvas-diff-bar-single");
        }

        let (
            lbl_compare,
            lbl_off,
            lbl_changed_only,
            lbl_render,
            lbl_after,
            lbl_before,
            lbl_overlay,
        ) = match self.lang {
            UiLang::Ko => (
                "비교",
                "비교 끄기",
                "변경된 화면만",
                "요소",
                "이후",
                "이전",
                "픽셀",
            ),
            UiLang::ZhCn => (
                "对比",
                "关闭对比",
                "仅看变更界面",
                "元素",
                "变更后",
                "变更前",
                "像素",
            ),
            UiLang::En => (
                "Compare",
                "No diff",
                "Changed only",
                "Elements",
                "After",
                "Before",
                "Pixel",
            ),
        };

        let same_project_other = self
            .workspace
            .runs
            .iter()
            .enumerate()
            .find(|(idx, r)| *idx != self.selected_run && r.project_id == run.project_id)
            .map(|(idx, _)| idx);
        let default_compare_idx = self
            .compare_run
            .filter(|idx| *idx != self.selected_run && *idx < self.workspace.runs.len())
            .or(same_project_other)
            .unwrap_or(other_runs[0].0);

        let compare_label = if self.canvas_diff_enabled {
            other_runs
                .iter()
                .find(|(idx, _)| *idx == default_compare_idx)
                .map(|(_, label)| format!("{lbl_compare}: {label}"))
                .unwrap_or_else(|| lbl_compare.to_owned())
        } else {
            format!("{lbl_compare}: {lbl_off}")
        };

        let changed_count = self
            .diff_entries
            .iter()
            .filter(|e| e.kind == "changed")
            .count();
        let added_count = self
            .diff_entries
            .iter()
            .filter(|e| e.kind == "added")
            .count();
        let removed_count = self
            .diff_entries
            .iter()
            .filter(|e| e.kind == "removed")
            .count();
        let summary_text = if self.diff_entries.is_empty() {
            match self.lang {
                UiLang::Ko => "UI 변경 없음".to_owned(),
                UiLang::ZhCn => "无界面变更".to_owned(),
                UiLang::En => "No UI changes".to_owned(),
            }
        } else {
            match self.lang {
                UiLang::Ko => {
                    format!("변경 {changed_count} · 추가 {added_count} · 삭제 {removed_count}")
                }
                UiLang::ZhCn => {
                    format!("变更 {changed_count} · 新增 {added_count} · 删除 {removed_count}")
                }
                UiLang::En => format!(
                    "{changed_count} changed · {added_count} added · {removed_count} removed"
                ),
            }
        };

        let next_compare_idx = if other_runs.len() > 1 {
            let pos = other_runs
                .iter()
                .position(|(idx, _)| *idx == default_compare_idx)
                .unwrap_or(0);
            other_runs[(pos + 1) % other_runs.len()].0
        } else {
            default_compare_idx
        };

        let enabled = self.canvas_diff_enabled;
        let changed_only = self.canvas_diff_changed_only;
        let mode = self.canvas_diff_mode;

        div()
            .id("desktop-canvas-diff-bar")
            .px_3()
            .py_1()
            .rounded_full()
            .border_1()
            .border_color(if enabled { p.primary } else { p.glass_line })
            .bg(p.glass)
            .shadow_md()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("canvas-diff-toggle-btn")
                    .px_2()
                    .py_0p5()
                    .rounded_full()
                    .border_1()
                    .border_color(if enabled { p.primary } else { p.glass_line })
                    .bg(if enabled { p.primary } else { p.glass_surface })
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            if !this.canvas_diff_enabled {
                                this.compare_run = Some(default_compare_idx);
                                this.canvas_diff_enabled = true;
                                this.ensure_diff_loaded();
                            } else {
                                this.canvas_diff_enabled = false;
                            }
                            cx.notify();
                        }),
                    )
                    .child(
                        Label::new(compare_label)
                            .text_xs()
                            .font_semibold()
                            .text_color(if enabled {
                                p.primary_foreground
                            } else {
                                p.foreground
                            }),
                    ),
            )
            .when(enabled && other_runs.len() > 1, |el| {
                el.child(
                    div()
                        .id("canvas-diff-cycle-run")
                        .px_2()
                        .py_0p5()
                        .rounded_full()
                        .border_1()
                        .border_color(p.glass_line)
                        .bg(p.glass_surface)
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.compare_run = Some(next_compare_idx);
                                this.diff_cache_key = None;
                                this.ensure_diff_loaded();
                                cx.notify();
                            }),
                        )
                        .child(Label::new("⇄").text_xs().text_color(p.foreground)),
                )
            })
            .when(enabled, |el| {
                el.child(
                    div()
                        .id("canvas-diff-changed-only-btn")
                        .px_2()
                        .py_0p5()
                        .rounded_full()
                        .border_1()
                        .border_color(if changed_only {
                            p.primary
                        } else {
                            p.glass_line
                        })
                        .bg(if changed_only {
                            p.primary
                        } else {
                            p.glass_surface
                        })
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.canvas_diff_changed_only = !this.canvas_diff_changed_only;
                                cx.notify();
                            }),
                        )
                        .child(
                            Label::new(lbl_changed_only)
                                .text_xs()
                                .font_semibold()
                                .text_color(if changed_only {
                                    p.primary_foreground
                                } else {
                                    p.muted_foreground
                                }),
                        ),
                )
                .child(
                    div().flex().items_center().gap_1().children(
                        [
                            (
                                "canvas-diff-mode-render",
                                CanvasDiffVisualMode::Render,
                                lbl_render,
                            ),
                            (
                                "canvas-diff-mode-after",
                                CanvasDiffVisualMode::After,
                                lbl_after,
                            ),
                            (
                                "canvas-diff-mode-before",
                                CanvasDiffVisualMode::Before,
                                lbl_before,
                            ),
                            (
                                "canvas-diff-mode-overlay",
                                CanvasDiffVisualMode::Overlay,
                                lbl_overlay,
                            ),
                        ]
                        .into_iter()
                        .map(|(id, target_mode, label)| {
                            let active = mode == target_mode;
                            div()
                                .id(id)
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .border_1()
                                .border_color(if active { p.primary } else { p.glass_line })
                                .bg(if active { p.primary } else { p.glass_surface })
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.canvas_diff_mode = target_mode;
                                        if target_mode == CanvasDiffVisualMode::Overlay {
                                            this.ensure_diff_loaded();
                                        }
                                        cx.notify();
                                    }),
                                )
                                .child(Label::new(label).text_xs().font_semibold().text_color(
                                    if active {
                                        p.primary_foreground
                                    } else {
                                        p.muted_foreground
                                    },
                                ))
                        }),
                    ),
                )
                .child(
                    Label::new(summary_text)
                        .text_xs()
                        .font_semibold()
                        .text_color(p.primary),
                )
            })
    }

    /// Full-bleed Overview Screen Map with focus-dimming for unrelated nodes when a screen is selected.
    fn render_overview_stage(
        &mut self,
        focus_active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = self.pal();
        let tr = i18n::strings(self.lang);
        let Some(run) = self.active_run().cloned() else {
            return div()
                .id("desktop-stage-empty")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new(tr.workspace_empty).text_color(p.muted_foreground));
        };

        let layout = compute_layout(&run.screens, self.selected_width);
        let open_by_path = self.workspace.open_comments_by_path(&run.project_id);
        let selected_idx = self.selected_screen;
        let mut related_indices: BTreeSet<usize> = BTreeSet::new();
        if focus_active {
            related_indices.insert(selected_idx);
            for edge in &layout.edges {
                if edge.from_idx == selected_idx {
                    related_indices.insert(edge.to_idx);
                }
                if edge.to_idx == selected_idx {
                    related_indices.insert(edge.from_idx);
                }
            }
        }

        if self.canvas_diff_enabled {
            self.ensure_diff_loaded();
        }
        let mut diff_by_path: BTreeMap<String, DiffCellEntry> = BTreeMap::new();
        if self.canvas_diff_enabled {
            for entry in &self.diff_entries {
                let replace = match diff_by_path.get(&entry.path) {
                    None => true,
                    Some(prev) => {
                        let prev_w_match = prev.width_px == self.selected_width;
                        let cur_w_match = entry.width_px == self.selected_width;
                        (cur_w_match && !prev_w_match)
                            || (cur_w_match == prev_w_match
                                && entry.overlay.is_some()
                                && prev.overlay.is_none())
                    }
                };
                if replace {
                    diff_by_path.insert(entry.path.clone(), entry.clone());
                }
            }
        }
        let canvas_diff_active = self.canvas_diff_enabled && !diff_by_path.is_empty();
        let canvas_diff_changed_only = self.canvas_diff_changed_only;
        let canvas_diff_mode = self.canvas_diff_mode;

        let mut decoded_cards: BTreeMap<usize, Option<Arc<RenderImage>>> = BTreeMap::new();
        for (idx, screen) in run.screens.iter().enumerate() {
            let img_opt = if let Some(diff_entry) = diff_by_path.get(&screen.path) {
                match canvas_diff_mode {
                    CanvasDiffVisualMode::Overlay => diff_entry
                        .overlay
                        .clone()
                        .or_else(|| {
                            diff_entry
                                .after_blob
                                .as_deref()
                                .and_then(|path| self.images.get_or_decode(path))
                        })
                        .or_else(|| {
                            self.cell_for_width(screen)
                                .and_then(|c| c.blob_path.as_deref())
                                .and_then(|path| self.images.get_or_decode(path))
                        }),
                    CanvasDiffVisualMode::Before => diff_entry
                        .before_blob
                        .as_deref()
                        .and_then(|path| self.images.get_or_decode(path))
                        .or_else(|| {
                            self.cell_for_width(screen)
                                .and_then(|c| c.blob_path.as_deref())
                                .and_then(|path| self.images.get_or_decode(path))
                        }),
                    CanvasDiffVisualMode::After | CanvasDiffVisualMode::Render => diff_entry
                        .after_blob
                        .as_deref()
                        .and_then(|path| self.images.get_or_decode(path))
                        .or_else(|| {
                            self.cell_for_width(screen)
                                .and_then(|c| c.blob_path.as_deref())
                                .and_then(|path| self.images.get_or_decode(path))
                        }),
                }
            } else {
                self.cell_for_width(screen)
                    .and_then(|c| c.blob_path.as_deref())
                    .and_then(|path| self.images.get_or_decode(path))
            };
            decoded_cards.insert(idx, img_opt);
        }

        let pan_x = self.pan_x;
        let pan_y = self.pan_y;
        let zoom = self.zoom;
        let metrics = layout.metrics;
        let card_w = (metrics.card_width * zoom).max(16.0);
        let pad_px = (card_w * 0.04).clamp(4.0, 12.0);
        let inner_w = (card_w - pad_px * 2.0).max(8.0);
        let caption_h = (metrics.caption_height * zoom).max(18.0);

        let mut node_rects: BTreeMap<usize, (f32, f32, f32, f32)> = BTreeMap::new();
        for node in &layout.nodes {
            let n_frame_h = (node.frame_height * zoom).max(10.0);
            let n_card_h = n_frame_h + caption_h + pad_px * 2.0;
            node_rects.insert(
                node.screen_idx,
                (pan_x + node.x * zoom, pan_y + node.y * zoom, n_frame_h, n_card_h),
            );
        }

        let edges_for_paint = layout.edges.clone();
        let node_rects_for_paint = node_rects.clone();
        let grid_color = p.map_grid;
        let link_color = p.map_link;
        let fg_color = p.foreground;
        let primary_color = p.primary;

        let edges_and_grid = canvas(
            |_bounds, _window, _cx| {},
            move |bounds: Bounds<Pixels>, _, window: &mut Window, _cx| {
                let ox = f32::from(bounds.origin.x);
                let oy = f32::from(bounds.origin.y);
                let bw = f32::from(bounds.size.width);
                let bh = f32::from(bounds.size.height);

                // 1. Exact 20px subtle dot grid matching `apps/canvas` `.viewer`
                let spacing = 20.0_f32;
                let sx0 = ((pan_x % spacing) + spacing) % spacing;
                let sy0 = ((pan_y % spacing) + spacing) % spacing;
                let mut gx = sx0;
                while gx < bw {
                    let mut gy = sy0;
                    while gy < bh {
                        let mut dot = PathBuilder::fill();
                        dot.add_polygon(
                            &[
                                point(px(ox + gx), px(oy + gy)),
                                point(px(ox + gx + 1.4), px(oy + gy)),
                                point(px(ox + gx + 1.4), px(oy + gy + 1.4)),
                                point(px(ox + gx), px(oy + gy + 1.4)),
                            ],
                            true,
                        );
                        if let Ok(path) = dot.build() {
                            window.paint_path(path, grid_color);
                        }
                        gy += spacing;
                    }
                    gx += spacing;
                }

                // 2. Orthogonal smooth-step connector edges; dim unrelated edges when focus is active
                let base_stroke_w = (1.6 * zoom.sqrt()).clamp(1.2, 2.5);
                for edge in &edges_for_paint {
                    let (Some(&(ax, ay, _, a_card_h)), Some(&(bx, by, _, b_card_h))) = (
                        node_rects_for_paint.get(&edge.from_idx),
                        node_rects_for_paint.get(&edge.to_idx),
                    ) else {
                        continue;
                    };
                    let connected_to_selected = focus_active
                        && (edge.from_idx == selected_idx || edge.to_idx == selected_idx);
                    let dimmed_edge = focus_active && !connected_to_selected;
                    let stroke_w = if connected_to_selected {
                        base_stroke_w * 1.45
                    } else {
                        base_stroke_w
                    };

                    let (sx, sy, tx, ty, horizontal) = if (bx - ax).abs() >= (by - ay).abs() {
                        if bx >= ax {
                            (ax + card_w, ay + a_card_h * 0.5, bx, by + b_card_h * 0.5, true)
                        } else {
                            (ax, ay + a_card_h * 0.5, bx + card_w, by + b_card_h * 0.5, true)
                        }
                    } else if by >= ay {
                        (ax + card_w * 0.5, ay + a_card_h, bx + card_w * 0.5, by, false)
                    } else {
                        (ax + card_w * 0.5, ay, bx + card_w * 0.5, by + b_card_h, false)
                    };

                    let is_verified = edge.source == "verified";
                    let is_inferred = edge.source == "inferred";
                    let mut pb = PathBuilder::stroke(px(stroke_w));
                    if is_inferred {
                        pb = pb.dash_array(&[px(3.0), px(4.0)]);
                    }
                    pb.move_to(point(px(ox + sx), px(oy + sy)));
                    if horizontal {
                        let mid_x = (sx + tx) * 0.5;
                        pb.line_to(point(px(ox + mid_x), px(oy + sy)));
                        pb.line_to(point(px(ox + mid_x), px(oy + ty)));
                        pb.line_to(point(px(ox + tx), px(oy + ty)));
                    } else {
                        let mid_y = (sy + ty) * 0.5;
                        pb.line_to(point(px(ox + sx), px(oy + mid_y)));
                        pb.line_to(point(px(ox + tx), px(oy + mid_y)));
                        pb.line_to(point(px(ox + tx), px(oy + ty)));
                    }
                    let mut color: Hsla = if is_verified {
                        primary_color
                    } else if connected_to_selected {
                        fg_color
                    } else {
                        link_color
                    };
                    if dimmed_edge {
                        color.a *= 0.14;
                    }
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, color);
                    }

                    let (ang_x, ang_y) = if horizontal {
                        (if tx >= sx { 1.0_f32 } else { -1.0_f32 }, 0.0_f32)
                    } else {
                        (0.0_f32, if ty >= sy { 1.0_f32 } else { -1.0_f32 })
                    };
                    let angle = ang_y.atan2(ang_x);
                    let hl = 6.0_f32;
                    let tip = point(px(ox + tx), px(oy + ty));
                    let left = point(
                        px(ox + tx - hl * (angle - 0.5).cos()),
                        px(oy + ty - hl * (angle - 0.5).sin()),
                    );
                    let right = point(
                        px(ox + tx - hl * (angle + 0.5).cos()),
                        px(oy + ty - hl * (angle + 0.5).sin()),
                    );
                    let mut arrow = PathBuilder::fill();
                    arrow.add_polygon(&[tip, left, right], true);
                    if let Ok(path) = arrow.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .size_full();

        let mut cards_layer = div().absolute().inset_0();
        for node in &layout.nodes {
            let idx = node.screen_idx;
            let screen = &run.screens[idx];
            let screen_diff = diff_by_path.get(&screen.path);
            if canvas_diff_active && canvas_diff_changed_only && screen_diff.is_none() {
                continue;
            }
            let (left, top, frame_h, card_h) = node_rects[&idx];
            let is_selected = focus_active && idx == selected_idx;
            let is_related = focus_active && !is_selected && related_indices.contains(&idx);
            let is_dimmed = (focus_active && !related_indices.contains(&idx))
                || (canvas_diff_active && screen_diff.is_none());
            let open_comments = open_by_path.get(&screen.path).copied().unwrap_or(0);
            let unverified = screen
                .cells
                .iter()
                .filter(|c| c.status_kind == "unverified")
                .count();
            let render_img = decoded_cards.get(&idx).and_then(|v| v.clone());

            let diff_badge_info = screen_diff.map(|d| {
                let (badge_bg, badge_label): (Hsla, String) = match d.kind.as_str() {
                    "added" => (
                        Hsla::from(gpui_kit::rgb(0x2f9e66)),
                        match self.lang {
                            UiLang::Ko => "추가됨".to_owned(),
                            UiLang::ZhCn => "新增".to_owned(),
                            UiLang::En => "Added".to_owned(),
                        },
                    ),
                    "removed" => (
                        Hsla::from(gpui_kit::rgb(0xd94848)),
                        match self.lang {
                            UiLang::Ko => "삭제됨".to_owned(),
                            UiLang::ZhCn => "已删除".to_owned(),
                            UiLang::En => "Removed".to_owned(),
                        },
                    ),
                    _ => {
                        let base = match self.lang {
                            UiLang::Ko => "변경됨",
                            UiLang::ZhCn => "已变更",
                            UiLang::En => "Changed",
                        };
                        let el_count = d
                            .render_diff
                            .as_ref()
                            .map(|rd| rd.changes.len())
                            .unwrap_or(0);
                        let text = if el_count > 0 {
                            match self.lang {
                                UiLang::Ko => format!("{base} · {el_count}요소"),
                                UiLang::ZhCn => format!("{base} · {el_count} 处元素"),
                                UiLang::En => format!("{base} · {el_count} el"),
                            }
                        } else {
                            match d.diff_percent {
                                Some(pct) if pct > 0.0 => format!("{base} · {pct:.1}%"),
                                _ => base.to_owned(),
                            }
                        };
                        (p.primary, text)
                    }
                };
                (badge_bg, badge_label)
            });

            let mut render_boxes: Vec<(f32, f32, f32, f32, Hsla)> = if canvas_diff_mode
                == CanvasDiffVisualMode::Render
                && let Some(d) = screen_diff
                && let Some(rd) = &d.render_diff
            {
                let cw = d.capture_width.max(1) as f32;
                let ch = d.capture_height.max(1) as f32;
                rd.changes
                    .iter()
                    .take(24)
                    .filter_map(|chg| {
                        let rect = chg.after_rect.or(chg.before_rect)?;
                        if rect.width <= 0 || rect.height <= 0 {
                            return None;
                        }
                        let rx = ((rect.x.max(0) as f32) / cw * inner_w).clamp(0.0, inner_w);
                        let ry = ((rect.y.max(0) as f32) / ch * frame_h).clamp(0.0, frame_h);
                        let rw = ((rect.width as f32) / cw * inner_w)
                            .clamp(2.0, (inner_w - rx).max(2.0));
                        let rh = ((rect.height as f32) / ch * frame_h)
                            .clamp(2.0, (frame_h - ry).max(2.0));
                        let color = match chg.kind {
                            dui_core::diff::RenderChangeKind::Added => {
                                Hsla::from(gpui_kit::rgb(0x2f9e66))
                            }
                            dui_core::diff::RenderChangeKind::Removed => {
                                Hsla::from(gpui_kit::rgb(0xd94848))
                            }
                            _ => p.primary,
                        };
                        Some((rx, ry, rw, rh, color))
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let screen_design_findings = self
                .design_audit
                .findings_by_screen_width
                .get(&(screen.path.clone(), self.selected_width))
                .cloned()
                .unwrap_or_default();
            let design_finding_count = screen_design_findings.len();
            if self.show_design_overlay && !screen_design_findings.is_empty() {
                let active_cell_dims = self
                    .cell_for_width(screen)
                    .and_then(|c| c.image_width.zip(c.image_height));
                let cw = active_cell_dims
                    .map(|(w, _)| w as f32)
                    .unwrap_or(self.selected_width.max(1) as f32)
                    .max(1.0);
                let ch = active_cell_dims
                    .map(|(_, h)| h as f32)
                    .unwrap_or(node.frame_height)
                    .max(1.0);
                for finding in screen_design_findings.iter().take(24) {
                    let rect = finding.rect;
                    if rect.width <= 0 || rect.height <= 0 {
                        continue;
                    }
                    let rx = ((rect.x.max(0) as f32) / cw * inner_w).clamp(0.0, inner_w);
                    let ry = ((rect.y.max(0) as f32) / ch * frame_h).clamp(0.0, frame_h);
                    let rw =
                        ((rect.width as f32) / cw * inner_w).clamp(2.0, (inner_w - rx).max(2.0));
                    let rh =
                        ((rect.height as f32) / ch * frame_h).clamp(2.0, (frame_h - ry).max(2.0));
                    let color =
                        Hsla::from(gpui_kit::rgb(design_severity_color_hex(finding.severity)));
                    render_boxes.push((rx, ry, rw, rh, color));
                }
            }

            let media_el = if let Some(ri) = render_img {
                let mut frame = div()
                    .relative()
                    .w(px(inner_w))
                    .h(px(frame_h))
                    .rounded_sm()
                    .overflow_hidden()
                    .bg(p.background)
                    .child(img(ImageSource::Render(ri)).w(px(inner_w)).h(px(frame_h)));
                for (rx, ry, rw, rh, mut border_col) in render_boxes {
                    let mut fill_col = border_col;
                    fill_col.a = 0.18;
                    border_col.a = 0.95;
                    frame = frame.child(
                        div()
                            .absolute()
                            .left(px(rx))
                            .top(px(ry))
                            .w(px(rw))
                            .h(px(rh))
                            .rounded_xs()
                            .border_2()
                            .border_color(border_col)
                            .bg(fill_col),
                    );
                }
                frame.when_some(diff_badge_info.clone(), |el, (badge_bg, badge_label)| {
                    el.child(
                        div()
                            .absolute()
                            .top(px(6.0))
                            .right(px(6.0))
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(badge_bg)
                            .shadow_sm()
                            .child(
                                Label::new(badge_label)
                                    .text_xs()
                                    .font_bold()
                                    .text_color(p.primary_foreground),
                            ),
                    )
                })
            } else {
                div()
                    .relative()
                    .w(px(inner_w))
                    .h(px(frame_h))
                    .rounded_sm()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(p.background)
                    .child(
                        Label::new("No image")
                            .text_xs()
                            .text_color(p.muted_foreground),
                    )
                    .when_some(diff_badge_info.clone(), |el, (badge_bg, badge_label)| {
                        el.child(
                            div()
                                .absolute()
                                .top(px(6.0))
                                .right(px(6.0))
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(badge_bg)
                                .child(
                                    Label::new(badge_label)
                                        .text_xs()
                                        .font_bold()
                                        .text_color(p.primary_foreground),
                                ),
                        )
                    })
            };

            let diff_border_color = diff_badge_info.as_ref().map(|(c, _)| *c);
            let card = div()
                .id(format!("map-card-{idx}"))
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(card_w))
                .h(px(card_h))
                .p(px(pad_px))
                .flex()
                .flex_col()
                .justify_between()
                .rounded_lg()
                .when(is_selected, |el| el.border_2().border_color(p.primary))
                .when(!is_selected && diff_border_color.is_some(), |el| {
                    el.border_2()
                        .border_color(diff_border_color.unwrap_or(p.primary))
                })
                .when(
                    !is_selected && diff_border_color.is_none() && is_related,
                    |el| el.border_1().border_color(p.foreground),
                )
                .when(
                    !is_selected && diff_border_color.is_none() && !is_related,
                    |el| el.border_1().border_color(p.capture_rim),
                )
                .when(is_dimmed, |el| el.opacity(0.22))
                .bg(p.capture_surface)
                .shadow_md()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.selected_screen = idx;
                        this.sync_cell_to_width();
                        this.neighbor_page = 0;
                        this.full_1x_detail = false;
                        this.comment_target = CommentTargetChoice::Screen;
                        this.view_mode = ViewMode::Detail;
                        cx.notify();
                    }),
                )
                .child(media_el)
                .child(
                    div()
                        .h(px(caption_h))
                        .pt_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_1()
                        .overflow_hidden()
                        .child(
                            Label::new(route_short_name(&screen.path))
                                .text_xs()
                                .font_semibold()
                                .text_color(p.foreground),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .when(design_finding_count > 0, |el| {
                                    el.child(
                                        Label::new(format!("⚠ {design_finding_count}"))
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(p.primary),
                                    )
                                })
                                .when(open_comments > 0, |el| {
                                    el.child(
                                        Label::new(format!("● {open_comments}"))
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(p.primary),
                                    )
                                })
                                .when(unverified > 0, |el| {
                                    el.child(
                                        Label::new(format!("{} {unverified}", tr.unverified))
                                            .text_xs()
                                            .text_color(p.muted_foreground),
                                    )
                                }),
                        ),
                );
            cards_layer = cards_layer.child(card);
        }

        // Bottom-left vertical zoom control stack matching React Flow `<Controls>` in `/tmp/obora-dui-overview.png`
        let ctrl_btn = |id: &'static str, text: &'static str| {
            div()
                .id(id)
                .w(px(28.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .border_1()
                .border_color(p.glass_line)
                .bg(p.glass)
                .cursor_pointer()
                .child(Label::new(text).text_xs().text_color(p.foreground))
        };

        let zoom_stack = div()
            .id("desktop-zoom-controls")
            .absolute()
            .left(px(12.0))
            .bottom(px(12.0))
            .flex()
            .flex_col()
            .gap_1()
            .child(ctrl_btn("zoom-in", "+").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.adjust_zoom(1.25);
                    cx.notify();
                }),
            ))
            .child(ctrl_btn("zoom-out", "−").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.adjust_zoom(0.8);
                    cx.notify();
                }),
            ))
            .child(ctrl_btn("zoom-fit", "⊡").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.fit_map();
                    cx.notify();
                }),
            ))
            .child(ctrl_btn("zoom-1x", "1:1").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.zoom_one_to_one();
                    cx.notify();
                }),
            ));

        // Bottom-right floating `Map options` pill matching `/tmp/obora-dui-overview.png`
        let map_options_btn = div()
            .id("map-options-btn")
            .absolute()
            .right(px(16.0))
            .bottom(px(14.0))
            .px_3()
            .py_2()
            .rounded_lg()
            .border_1()
            .border_color(p.glass_line)
            .bg(p.glass)
            .shadow_md()
            .cursor_pointer()
            .flex()
            .items_center()
            .gap_2()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.options_open = !this.options_open;
                    this.picker_open = false;
                    cx.notify();
                }),
            )
            .child(Label::new("⊶").text_xs().text_color(p.muted_foreground))
            .child(
                Label::new(tr.map_options)
                    .text_xs()
                    .text_color(p.foreground),
            );

        // Top-center floating audit pill + inline canvas diff bar (matching `apps/canvas` `.map-top-bar`)
        let top_pill = div()
            .absolute()
            .top(px(12.0))
            .left_0()
            .right_0()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_center()
            .gap_2()
            .child(self.render_live_surface_pill(cx))
            .child(self.render_audit_pill(cx))
            .child(self.render_canvas_diff_bar(cx));

        div()
            .id("desktop-map-viewport")
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .on_scroll(cx.listener(|this, event: &ScrollEvent, _, cx| {
                let delta = match event.delta {
                    ScrollDelta::Pixels(pt) => (f32::from(pt.x), f32::from(pt.y)),
                    ScrollDelta::Lines(l) => (l.x * 24.0, l.y * 24.0),
                };
                if event.modifiers.platform || event.modifiers.control {
                    this.adjust_zoom(if delta.1 > 0.0 { 1.1 } else { 0.9 });
                } else {
                    this.pan_x += delta.0;
                    this.pan_y += delta.1;
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, _| {
                    this.dragging = true;
                    this.drag_moved = false;
                    this.drag_last = event.position;
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.dragging {
                    let dx = f32::from(event.position.x) - f32::from(this.drag_last.x);
                    let dy = f32::from(event.position.y) - f32::from(this.drag_last.y);
                    if dx.abs() > 2.0 || dy.abs() > 2.0 {
                        this.drag_moved = true;
                    }
                    this.pan_x += dx;
                    this.pan_y += dy;
                    this.drag_last = event.position;
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                    let was_click = this.dragging && !this.drag_moved;
                    this.dragging = false;
                    this.drag_moved = false;
                    if was_click && focus_active {
                        this.full_1x_detail = false;
                        this.view_mode = ViewMode::Overview;
                        cx.notify();
                    }
                }),
            )
            .child(edges_and_grid)
            .child(cards_layer)
            .child(top_pill)
            .child(zoom_stack)
            .child(map_options_btn)
    }

    /// Screen Neighborhood + Floating Glass Right Inspector matching `/tmp/obora-dui-detail.png` 1:1.
    fn render_detail_stage(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.pal();
        let tr = i18n::strings(self.lang);
        let Some(run) = self.active_run().cloned() else {
            return div()
                .id("detail-empty")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new(tr.workspace_empty).text_color(p.muted_foreground));
        };
        let Some(screen) = run.screens.get(self.selected_screen).cloned() else {
            return div().id("detail-no-screen").flex_1().h_full();
        };

        let active_cell = self.active_cell().cloned();
        let focus_img = active_cell
            .as_ref()
            .and_then(|c| c.blob_path.as_deref())
            .and_then(|path| self.images.get_or_decode(path));

        // Connected partner screens (outgoing + incoming)
        let mut partner_indices: Vec<usize> = Vec::new();
        for edge in &screen.outgoing {
            if let Some(idx) = run.screens.iter().position(|s| s.path == edge.to_path)
                && idx != self.selected_screen
                && !partner_indices.contains(&idx)
            {
                partner_indices.push(idx);
            }
        }
        for (idx, other) in run.screens.iter().enumerate() {
            if idx != self.selected_screen
                && !partner_indices.contains(&idx)
                && other.outgoing.iter().any(|e| e.to_path == screen.path)
            {
                partner_indices.push(idx);
            }
        }

        let total_partners = partner_indices.len();
        let per_page = 2usize;
        let max_page = total_partners.saturating_sub(1) / per_page;
        if self.neighbor_page > max_page {
            self.neighbor_page = 0;
        }
        let page_start = self.neighbor_page * per_page;
        let visible_partners: Vec<usize> = partner_indices
            .iter()
            .skip(page_start)
            .take(per_page)
            .copied()
            .collect();

        let mut partner_imgs: Vec<(usize, String, Option<Arc<RenderImage>>)> = Vec::new();
        for &p_idx in &visible_partners {
            let ps = &run.screens[p_idx];
            let ri = self
                .cell_for_width(ps)
                .and_then(|c| c.blob_path.as_deref())
                .and_then(|path| self.images.get_or_decode(path));
            partner_imgs.push((p_idx, ps.path.clone(), ri));
        }

        let grid_color = p.map_grid;
        let link_color = p.map_link;
        let partner_count = partner_imgs.len();
        let full_1x = self.full_1x_detail;

        // Dot grid + orthogonal connectors between main card and right partner cards
        let bg_canvas = canvas(
            |_bounds, _window, _cx| {},
            move |bounds: Bounds<Pixels>, _, window: &mut Window, _cx| {
                let ox = f32::from(bounds.origin.x);
                let oy = f32::from(bounds.origin.y);
                let bw = f32::from(bounds.size.width);
                let bh = f32::from(bounds.size.height);

                let spacing = 20.0_f32;
                let mut gx = 10.0_f32;
                while gx < bw {
                    let mut gy = 10.0_f32;
                    while gy < bh {
                        let mut dot = PathBuilder::fill();
                        dot.add_polygon(
                            &[
                                point(px(ox + gx), px(oy + gy)),
                                point(px(ox + gx + 1.4), px(oy + gy)),
                                point(px(ox + gx + 1.4), px(oy + gy + 1.4)),
                                point(px(ox + gx), px(oy + gy + 1.4)),
                            ],
                            true,
                        );
                        if let Ok(path) = dot.build() {
                            window.paint_path(path, grid_color);
                        }
                        gy += spacing;
                    }
                    gx += spacing;
                }

                if !full_1x && partner_count > 0 {
                    // Clean orthogonal connector from focused card center-right to partner stack
                    let mid_y = oy + bh * 0.5;
                    let left_x = ox + (bw * 0.48).min(520.0);
                    let right_x = left_x + 76.0;
                    let fork_x = (left_x + right_x) * 0.5;
                    let targets: Vec<f32> = if partner_count == 1 {
                        vec![mid_y]
                    } else {
                        vec![mid_y - 135.0, mid_y + 135.0]
                    };
                    for ty in targets {
                        let mut pb = PathBuilder::stroke(px(1.5));
                        pb.move_to(point(px(left_x), px(mid_y)));
                        pb.line_to(point(px(fork_x), px(mid_y)));
                        pb.line_to(point(px(fork_x), px(ty)));
                        pb.line_to(point(px(right_x), px(ty)));
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, link_color);
                        }
                    }
                }
            },
        )
        .size_full();

        let center_content = if full_1x {
            let full_preview = if let Some(ri) = focus_img {
                let dims = ri.size(0);
                let w = (dims.width.0 as f32).min(1280.0);
                let h = (dims.height.0 as f32).min(2400.0);
                div()
                    .id("full-1x-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .p_6()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .p_2()
                            .rounded_xl()
                            .border_1()
                            .border_color(p.capture_rim)
                            .bg(p.capture_surface)
                            .shadow_lg()
                            .child(img(ImageSource::Render(ri)).w(px(w)).h(px(h))),
                    )
            } else {
                div()
                    .id("full-1x-empty")
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Label::new("No image").text_color(p.muted_foreground))
            };
            div().absolute().inset_0().child(full_preview)
        } else {
            let focus_w = if active_cell.as_ref().is_some_and(|c| c.width_px <= 480) {
                320.0_f32
            } else {
                440.0_f32
            };
            let focus_h = if active_cell.as_ref().is_some_and(|c| c.width_px <= 480) {
                460.0_f32
            } else {
                275.0_f32
            };
            let focus_card = div()
                .id("neighborhood-focus-card")
                .w(px(focus_w + 20.0))
                .p(px(10.0))
                .rounded_xl()
                .border_1()
                .border_color(p.capture_rim)
                .bg(p.capture_surface)
                .shadow_lg()
                .flex()
                .flex_col()
                .gap_2()
                .child(if let Some(ri) = focus_img {
                    div()
                        .w(px(focus_w))
                        .h(px(focus_h))
                        .rounded_md()
                        .overflow_hidden()
                        .bg(p.background)
                        .child(img(ImageSource::Render(ri)).w(px(focus_w)).h(px(focus_h)))
                } else {
                    div()
                        .w(px(focus_w))
                        .h(px(focus_h))
                        .rounded_md()
                        .bg(p.background)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Label::new("No image").text_color(p.muted_foreground))
                })
                .child(
                    Label::new(route_short_name(&screen.path))
                        .text_sm()
                        .font_semibold()
                        .text_color(p.foreground),
                );

            let mut partners_col = div().flex().flex_col().gap_5();
            for (p_idx, p_path, p_ri) in partner_imgs {
                let pw = 300.0_f32;
                let ph = 188.0_f32;
                partners_col = partners_col.child(
                    div()
                        .id(format!("neighbor-partner-{p_idx}"))
                        .w(px(pw + 16.0))
                        .p(px(8.0))
                        .rounded_xl()
                        .border_1()
                        .border_color(p.capture_rim)
                        .bg(p.capture_surface)
                        .shadow_md()
                        .cursor_pointer()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.selected_screen = p_idx;
                                this.sync_cell_to_width();
                                this.neighbor_page = 0;
                                this.comment_target = CommentTargetChoice::Screen;
                                cx.notify();
                            }),
                        )
                        .child(if let Some(ri) = p_ri {
                            div()
                                .w(px(pw))
                                .h(px(ph))
                                .rounded_md()
                                .overflow_hidden()
                                .bg(p.background)
                                .child(img(ImageSource::Render(ri)).w(px(pw)).h(px(ph)))
                        } else {
                            div()
                                .w(px(pw))
                                .h(px(ph))
                                .rounded_md()
                                .bg(p.background)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Label::new("No image")
                                        .text_xs()
                                        .text_color(p.muted_foreground),
                                )
                        })
                        .child(
                            Label::new(route_short_name(&p_path))
                                .text_xs()
                                .font_semibold()
                                .text_color(p.foreground),
                        ),
                );
            }

            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(76.0))
                .child(focus_card)
                .when(total_partners > 0, |el| el.child(partners_col))
        };

        // Bottom-left pagination bar matching `/tmp/obora-dui-detail.png` (`Connected screens · 1–2 of 3 < >`)
        let pagination = if total_partners > 0 && !full_1x {
            let from_n = page_start + 1;
            let to_n = (page_start + visible_partners.len()).min(total_partners);
            div()
                .id("neighborhood-pagination")
                .absolute()
                .left(px(16.0))
                .bottom(px(16.0))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Label::new(format!(
                        "{} · {from_n}–{to_n} of {total_partners}",
                        tr.connected_screens
                    ))
                    .text_xs()
                    .text_color(p.muted_foreground),
                )
                .child(
                    div()
                        .id("neighbor-prev")
                        .w(px(24.0))
                        .h(px(24.0))
                        .rounded_md()
                        .border_1()
                        .border_color(p.glass_line)
                        .bg(p.glass)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.neighbor_page = this.neighbor_page.saturating_sub(1);
                                cx.notify();
                            }),
                        )
                        .child(Label::new("‹").text_xs().text_color(p.foreground)),
                )
                .child(
                    div()
                        .id("neighbor-next")
                        .w(px(24.0))
                        .h(px(24.0))
                        .rounded_md()
                        .border_1()
                        .border_color(p.glass_line)
                        .bg(p.glass)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                if this.neighbor_page < max_page {
                                    this.neighbor_page += 1;
                                }
                                cx.notify();
                            }),
                        )
                        .child(Label::new("›").text_xs().text_color(p.foreground)),
                )
        } else {
            div().id("neighborhood-pagination-empty")
        };

        let map_options_btn = div()
            .id("detail-map-options-btn")
            .absolute()
            .right(px(16.0))
            .bottom(px(14.0))
            .px_3()
            .py_2()
            .rounded_lg()
            .border_1()
            .border_color(p.glass_line)
            .bg(p.glass)
            .shadow_md()
            .cursor_pointer()
            .flex()
            .items_center()
            .gap_2()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.options_open = !this.options_open;
                    this.picker_open = false;
                    cx.notify();
                }),
            )
            .child(Label::new("⊶").text_xs().text_color(p.muted_foreground))
            .child(
                Label::new(tr.map_options)
                    .text_xs()
                    .text_color(p.foreground),
            );

        let neighborhood_area = div()
            .id("detail-neighborhood-area")
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .child(bg_canvas)
            .child(center_content)
            .child(pagination)
            .child(map_options_btn);

        let inspector = self.render_floating_inspector(&run, &screen, cx);

        div()
            .id("desktop-detail-layout")
            .relative()
            .flex_1()
            .h_full()
            .flex()
            .flex_row()
            .child(neighborhood_area)
            .child(inspector)
    }

    /// Floating right glass ScreenInspector matching `/tmp/obora-dui-detail.png` 1:1.
    fn render_floating_inspector(
        &self,
        run: &RunItem,
        screen: &ScreenItem,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = self.pal();
        let tr = i18n::strings(self.lang);

        let status_label = |kind: &str| match kind {
            "verified" => tr.verified,
            "unrenderable" => tr.unrenderable,
            _ => tr.unverified,
        };

        let mut cells_col = div().flex().flex_col().gap_2();
        for (c_idx, entry) in screen.cells.iter().enumerate() {
            let active = c_idx == self.selected_cell;
            let entry_w = entry.width_px;
            let st_text = status_label(&entry.status_kind);
            cells_col = cells_col.child(
                div()
                    .id(format!("insp-cell-{c_idx}"))
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(if active { p.foreground } else { p.glass_line })
                    .bg(p.glass_surface)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            let width_changed = this.selected_width != entry_w;
                            this.selected_cell = c_idx;
                            this.selected_width = entry_w;
                            if width_changed {
                                this.fit_map();
                            }
                            cx.notify();
                        }),
                    )
                    .child(
                        Label::new(format!("{} px", entry.width_px))
                            .text_xs()
                            .font_semibold()
                            .text_color(p.foreground),
                    )
                    .child(Label::new(st_text).text_xs().text_color(p.muted_foreground)),
            );
        }

        let mut links_col = div().flex().flex_col().gap_2();
        if screen.outgoing.is_empty() {
            links_col = links_col.child(
                Label::new(tr.no_links)
                    .text_xs()
                    .text_color(p.muted_foreground),
            );
        } else {
            for (e_idx, edge) in screen.outgoing.iter().take(6).enumerate() {
                let target_idx = run.screens.iter().position(|s| s.path == edge.to_path);
                let row = div()
                    .id(format!("insp-edge-{e_idx}"))
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(p.glass_line)
                    .bg(p.glass_surface)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .overflow_hidden()
                    .child(
                        Label::new(format!("↗ {}", edge.to_path))
                            .text_xs()
                            .text_color(p.foreground),
                    )
                    .when_some(target_idx, |el, t_idx| {
                        el.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.selected_screen = t_idx;
                                this.sync_cell_to_width();
                                this.neighbor_page = 0;
                                this.comment_target = CommentTargetChoice::Screen;
                                cx.notify();
                            }),
                        )
                    });
                links_col = links_col.child(row);
            }
        }

        // Comment target pills
        let active_w = self
            .active_cell()
            .map(|c| c.width_px)
            .unwrap_or(self.selected_width);
        let mut target_bar = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(
                Button::new("cmt-target-screen")
                    .label(tr.target_screen)
                    .when(self.comment_target == CommentTargetChoice::Screen, |b| {
                        b.primary()
                    })
                    .when(self.comment_target != CommentTargetChoice::Screen, |b| {
                        b.outline()
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.comment_target = CommentTargetChoice::Screen;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("cmt-target-cell")
                    .label(format!("{} {active_w}px", tr.target_width))
                    .when(self.comment_target == CommentTargetChoice::Cell, |b| {
                        b.primary()
                    })
                    .when(self.comment_target != CommentTargetChoice::Cell, |b| {
                        b.outline()
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.comment_target = CommentTargetChoice::Cell;
                        cx.notify();
                    })),
            );
        if let Some(first_edge) = screen.outgoing.first() {
            let to = first_edge.to_path.clone();
            let active = self.comment_target == CommentTargetChoice::Transition(to.clone());
            target_bar = target_bar.child(
                Button::new("cmt-target-transition")
                    .label(format!(
                        "{} → {}",
                        tr.target_transition,
                        route_short_name(&to)
                    ))
                    .when(active, |b| b.primary())
                    .when(!active, |b| b.outline())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.comment_target = CommentTargetChoice::Transition(to.clone());
                        cx.notify();
                    })),
            );
        }

        let matching_comments: Vec<_> = self
            .workspace
            .comments
            .iter()
            .filter(|c| {
                c.project_id == run.project_id
                    && (c.screen_path.as_deref() == Some(screen.path.as_str())
                        || c.screen_path.is_none())
            })
            .cloned()
            .collect();

        let mut comments_col = div().flex().flex_col().gap_2();
        if matching_comments.is_empty() {
            comments_col = comments_col.child(
                Label::new(tr.no_comments)
                    .text_xs()
                    .text_color(p.muted_foreground),
            );
        } else {
            for (idx, c) in matching_comments.iter().enumerate() {
                let cid = c.id.clone();
                let is_open = c.status == "open";
                comments_col = comments_col.child(
                    div()
                        .id(format!("desktop-comment-{idx}"))
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(p.glass_line)
                        .bg(p.glass_surface)
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    Label::new(format!("{} · {}", c.target_label, c.status))
                                        .text_xs()
                                        .text_color(p.muted_foreground),
                                )
                                .child(
                                    Button::new(format!("toggle-comment-{idx}"))
                                        .label(if is_open { tr.resolve } else { tr.reopen })
                                        .outline()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.toggle_comment(cid.clone(), cx);
                                        })),
                                ),
                        )
                        .child(
                            Label::new(c.body.clone())
                                .text_xs()
                                .text_color(p.foreground),
                        )
                        .child(
                            Label::new(c.author.clone())
                                .text_xs()
                                .text_color(p.muted_foreground),
                        ),
                );
            }
        }

        let title = route_short_name(&screen.path);
        let multiple_states = screen.cells.len() > 2;
        let active_cell = self.active_cell().cloned();
        let live_available = run.project_path.is_some()
            && run.run_dir.is_some()
            && active_cell
                .as_ref()
                .is_some_and(|c| !c.state_id.is_empty() && c.status_kind != "unrenderable");

        let screen_diff_entry = if self.canvas_diff_enabled {
            self.diff_entries
                .iter()
                .find(|e| e.path == screen.path && e.width_px == self.selected_width)
                .or_else(|| self.diff_entries.iter().find(|e| e.path == screen.path))
                .cloned()
        } else {
            None
        };
        let inspector_diff_box = screen_diff_entry.map(|d| {
            let before_ri = d
                .before_blob
                .as_deref()
                .and_then(store::decode_png_to_render_image);
            let after_ri = d
                .after_blob
                .as_deref()
                .and_then(store::decode_png_to_render_image);
            let overlay_ri = d.overlay.clone();
            let ratio_text = d
                .diff_percent
                .filter(|v| *v > 0.0)
                .map(|v| format!("{v:.1}% px"))
                .unwrap_or_default();
            let kind_text = match (self.lang, d.kind.as_str()) {
                (UiLang::Ko, "added") => "추가됨",
                (UiLang::Ko, "removed") => "삭제됨",
                (UiLang::Ko, _) => "변경됨",
                (UiLang::ZhCn, "added") => "新增",
                (UiLang::ZhCn, "removed") => "已删除",
                (UiLang::ZhCn, _) => "已变更",
                (UiLang::En, "added") => "Added",
                (UiLang::En, "removed") => "Removed",
                (UiLang::En, _) => "Changed",
            };
            let mut thumbs = div().flex().items_center().gap_2();
            if let Some(ri) = before_ri {
                thumbs = thumbs.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            Label::new(tr.before)
                                .text_xs()
                                .text_color(p.muted_foreground),
                        )
                        .child(
                            div()
                                .h(px(64.0))
                                .rounded_sm()
                                .overflow_hidden()
                                .border_1()
                                .border_color(p.glass_line)
                                .child(img(ImageSource::Render(ri)).w_full().h_full()),
                        ),
                );
            }
            if let Some(ri) = after_ri {
                thumbs = thumbs.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            Label::new(tr.after)
                                .text_xs()
                                .text_color(p.muted_foreground),
                        )
                        .child(
                            div()
                                .h(px(64.0))
                                .rounded_sm()
                                .overflow_hidden()
                                .border_1()
                                .border_color(p.glass_line)
                                .child(img(ImageSource::Render(ri)).w_full().h_full()),
                        ),
                );
            }
            if let Some(ri) = overlay_ri {
                thumbs = thumbs.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            Label::new(match self.lang {
                                UiLang::Ko => "오버레이",
                                UiLang::ZhCn => "差异叠加",
                                UiLang::En => "Overlay",
                            })
                            .text_xs()
                            .text_color(p.muted_foreground),
                        )
                        .child(
                            div()
                                .h(px(64.0))
                                .rounded_sm()
                                .overflow_hidden()
                                .border_1()
                                .border_color(p.primary)
                                .child(img(ImageSource::Render(ri)).w_full().h_full()),
                        ),
                );
            }
            let mut render_changes_col = div().flex().flex_col().gap_1();
            let has_render_changes = d
                .render_diff
                .as_ref()
                .is_some_and(|rd| !rd.changes.is_empty());
            if let Some(rd) = &d.render_diff {
                for chg in rd.changes.iter().take(6) {
                    let accent = match chg.kind {
                        dui_core::diff::RenderChangeKind::Added => {
                            Hsla::from(gpui_kit::rgb(0x2f9e66))
                        }
                        dui_core::diff::RenderChangeKind::Removed => {
                            Hsla::from(gpui_kit::rgb(0xd94848))
                        }
                        _ => p.primary,
                    };
                    render_changes_col = render_changes_col.child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .border_l_2()
                            .border_color(accent)
                            .bg(p.background)
                            .child(
                                Label::new(chg.summary.clone())
                                    .text_xs()
                                    .text_color(p.foreground),
                            ),
                    );
                }
            }
            div()
                .id("inspector-inline-diff")
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(p.primary)
                .bg(p.glass_surface)
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            Label::new(kind_text)
                                .text_xs()
                                .font_bold()
                                .text_color(p.primary),
                        )
                        .when(!ratio_text.is_empty(), |el| {
                            el.child(
                                Label::new(ratio_text)
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(p.foreground),
                            )
                        }),
                )
                .child(thumbs)
                .when(has_render_changes, |el| el.child(render_changes_col))
        });

        let render_nodes = self
            .design_audit
            .trees_by_screen_width
            .get(&(screen.path.clone(), self.selected_width))
            .cloned()
            .unwrap_or_default();
        let selected_node = self
            .selected_element_id
            .as_deref()
            .and_then(|id| render_nodes.iter().find(|n| n.id == id))
            .or_else(|| render_nodes.first())
            .cloned();

        let element_inspector_box = selected_node.map(|node| {
            let (title, bm_label, attr_label) = match self.lang {
                UiLang::Ko => ("요소 인스펙터", "박스 모델", "속성"),
                UiLang::ZhCn => ("元素检查器", "盒模型", "属性"),
                UiLang::En => ("Element inspector", "Box model", "Attributes"),
            };
            let bm = node.box_model.clone().unwrap_or_default();
            let content_w = (node.rect.width
                - bm.padding.left
                - bm.padding.right
                - bm.border.left
                - bm.border.right)
                .max(0);
            let content_h = (node.rect.height
                - bm.padding.top
                - bm.padding.bottom
                - bm.border.top
                - bm.border.bottom)
                .max(0);

            let mut pills_row = div().flex().flex_wrap().gap_1();
            for item in render_nodes.iter().take(12) {
                let is_active = item.id == node.id;
                let item_id = item.id.clone();
                pills_row = pills_row.child(
                    div()
                        .id(format!("el-pick-{}", item.id))
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .border_1()
                        .border_color(if is_active { p.primary } else { p.glass_line })
                        .bg(if is_active { p.primary } else { p.background })
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.selected_element_id = Some(item_id.clone());
                                cx.notify();
                            }),
                        )
                        .child(
                            Label::new(format!(
                                "{} ({}×{})",
                                item.id, item.rect.width, item.rect.height
                            ))
                            .text_xs()
                            .text_color(if is_active {
                                p.primary_foreground
                            } else {
                                p.foreground
                            }),
                        ),
                );
            }

            let mut props_col = div().flex().flex_col().gap_0p5();
            if let Some(gap) = &bm.gap {
                props_col = props_col.child(
                    Label::new(format!("gap: {gap}"))
                        .text_xs()
                        .text_color(p.muted_foreground),
                );
            }
            if let Some(radius) = &bm.border_radius {
                props_col = props_col.child(
                    Label::new(format!("border-radius: {radius}"))
                        .text_xs()
                        .text_color(p.muted_foreground),
                );
            }
            for (k, v) in node.style.iter().take(6) {
                props_col = props_col.child(
                    Label::new(format!("{k}: {v}"))
                        .text_xs()
                        .text_color(p.muted_foreground),
                );
            }
            if !node.attributes.is_empty() {
                props_col = props_col.child(
                    Label::new(format!(
                        "{attr_label}: {}",
                        node.attributes
                            .iter()
                            .take(4)
                            .map(|(k, v)| format!("{k}={v}"))
                            .collect::<Vec<_>>()
                            .join(" · ")
                    ))
                    .text_xs()
                    .text_color(p.foreground),
                );
            }

            let margin_bg = Hsla {
                a: 0.16,
                ..Hsla::from(gpui_kit::rgb(0xf59e0b))
            };
            let padding_bg = Hsla {
                a: 0.16,
                ..Hsla::from(gpui_kit::rgb(0x10b981))
            };
            let content_bg = Hsla {
                a: 0.20,
                ..Hsla::from(gpui_kit::rgb(0x3b82f6))
            };

            div()
                .id("desktop-element-inspector")
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(p.glass_line)
                .bg(p.glass_surface)
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            Label::new(format!("{title} ({})", render_nodes.len()))
                                .text_xs()
                                .font_semibold()
                                .text_color(p.foreground),
                        )
                        .child(
                            Label::new(format!(
                                "{}×{} px · ({}, {})",
                                node.rect.width, node.rect.height, node.rect.x, node.rect.y
                            ))
                            .text_xs()
                            .text_color(p.muted_foreground),
                        ),
                )
                .child(pills_row)
                // Nested DevTools Box-Model diagram (Margin -> Padding -> Content)
                .child(
                    div()
                        .p_1p5()
                        .rounded_sm()
                        .border_1()
                        .border_color(Hsla::from(gpui_kit::rgb(0xf59e0b)))
                        .bg(margin_bg)
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            Label::new(format!(
                                "{bm_label} · margin {} {} {} {}",
                                bm.margin.top, bm.margin.right, bm.margin.bottom, bm.margin.left
                            ))
                            .text_xs()
                            .text_color(p.foreground),
                        )
                        .child(
                            div()
                                .p_1p5()
                                .rounded_sm()
                                .border_1()
                                .border_color(Hsla::from(gpui_kit::rgb(0x10b981)))
                                .bg(padding_bg)
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    Label::new(format!(
                                        "padding {} {} {} {}",
                                        bm.padding.top,
                                        bm.padding.right,
                                        bm.padding.bottom,
                                        bm.padding.left
                                    ))
                                    .text_xs()
                                    .text_color(p.foreground),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(Hsla::from(gpui_kit::rgb(0x3b82f6)))
                                        .bg(content_bg)
                                        .flex()
                                        .justify_center()
                                        .child(
                                            Label::new(format!(
                                                "{} · {content_w} × {content_h} px",
                                                node.id
                                            ))
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(p.foreground),
                                        ),
                                ),
                        ),
                )
                .child(props_col)
        });

        let screen_findings = self
            .design_audit
            .findings_by_screen_width
            .get(&(screen.path.clone(), self.selected_width))
            .cloned()
            .unwrap_or_default();
        let design_findings_box = (!screen_findings.is_empty()).then(|| {
            let heading = match self.lang {
                UiLang::Ko => format!("디자인 · 접근성 진단 ({})", screen_findings.len()),
                UiLang::ZhCn => format!("设计与无障碍检查 ({})", screen_findings.len()),
                UiLang::En => format!("Design & a11y checks ({})", screen_findings.len()),
            };
            let mut list = div()
                .id("desktop-design-findings")
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(p.primary)
                .bg(p.glass_surface)
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Label::new(heading)
                        .text_xs()
                        .font_semibold()
                        .text_color(p.primary),
                );
            for f in screen_findings.iter().take(8) {
                let accent = Hsla::from(gpui_kit::rgb(design_severity_color_hex(f.severity)));
                list = list.child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_l_2()
                        .border_color(accent)
                        .bg(p.background)
                        .flex()
                        .flex_col()
                        .child(
                            Label::new(f.summary.clone())
                                .text_xs()
                                .text_color(p.foreground),
                        )
                        .child(
                            Label::new(format!("{} · {}", f.measured, f.expected))
                                .text_xs()
                                .text_color(p.muted_foreground),
                        ),
                );
            }
            list
        });

        let diagnostic_box = active_cell.as_ref().and_then(|c| {
            let show_reason = c.status_kind != "verified";
            let path_diff = c
                .observed_path
                .as_deref()
                .filter(|obs| !obs.is_empty() && *obs != screen.path);
            if !show_reason && c.http_errors.is_empty() && path_diff.is_none() {
                return None;
            }
            let mut box_el = div()
                .id("inspector-cell-diagnostics")
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(if show_reason { p.primary } else { p.glass_line })
                .bg(p.glass_surface)
                .flex()
                .flex_col()
                .gap_1();
            if show_reason {
                let reason_text = c.reason.as_deref().unwrap_or("missing-evidence");
                box_el = box_el.child(
                    Label::new(format!("{} · {reason_text}", status_label(&c.status_kind)))
                        .text_xs()
                        .font_semibold()
                        .text_color(p.primary),
                );
            }
            for err_line in &c.http_errors {
                box_el = box_el.child(
                    Label::new(err_line.clone())
                        .text_xs()
                        .text_color(p.foreground),
                );
            }
            if let Some(obs) = path_diff {
                box_el = box_el.child(
                    Label::new(format!("URL: {obs}"))
                        .text_xs()
                        .text_color(p.muted_foreground),
                );
            }
            Some(box_el)
        });

        // Floating glass panel (margin: 10px, rounded-xl) matching `/tmp/obora-dui-detail.png`!
        div()
            .id("floating-inspector-wrapper")
            .w(px(332.0))
            .h_full()
            .flex_shrink_0()
            .p_2()
            .child(
                div()
                    .id("desktop-inspector")
                    .size_full()
                    .p_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(p.glass_line)
                    .bg(p.glass)
                    .shadow_lg()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        Label::new(title)
                                            .text_base()
                                            .font_bold()
                                            .text_color(p.foreground),
                                    )
                                    .child(
                                        Label::new(screen.path.clone())
                                            .text_xs()
                                            .text_color(p.muted_foreground),
                                    )
                                    .child(
                                        Label::new(tr.archive)
                                            .text_xs()
                                            .text_color(p.muted_foreground),
                                    ),
                            )
                            .child(
                                div()
                                    .id("inspector-close-btn")
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .rounded_md()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.full_1x_detail = false;
                                            this.view_mode = ViewMode::Overview;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        Label::new("✕").text_sm().text_color(p.muted_foreground),
                                    ),
                            ),
                    )
                    // Brand orange `#ef803f` `Open live` CTA matching `apps/canvas` + `1:1` inspect toggle
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .id("inspector-live-cta")
                                    .w_full()
                                    .py_2()
                                    .rounded_md()
                                    .bg(p.primary)
                                    .when(!live_available, |el| el.opacity(0.45))
                                    .when(live_available, |el| el.cursor_pointer())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .when(live_available, |el| {
                                        el.on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.open_live_session(cx);
                                            }),
                                        )
                                    })
                                    .child(
                                        Label::new(format!("▶ {}", tr.open_live))
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(p.primary_foreground),
                                    ),
                            )
                            .child(
                                Button::new("inspector-1x-toggle")
                                    .label(if self.full_1x_detail {
                                        tr.back_to_overview
                                    } else {
                                        tr.inspect_1x
                                    })
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.full_1x_detail = !this.full_1x_detail;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(div().h(px(1.0)).w_full().bg(p.glass_line))
                    .child(
                        Label::new(if multiple_states {
                            tr.states_and_widths
                        } else {
                            tr.widths
                        })
                        .text_xs()
                        .text_color(p.muted_foreground),
                    )
                    .child(cells_col)
                    .when_some(inspector_diff_box, |el, diff_box| el.child(diff_box))
                    .when_some(element_inspector_box, |el, el_box| el.child(el_box))
                    .when_some(design_findings_box, |el, df_box| el.child(df_box))
                    .when_some(diagnostic_box, |el, diag| el.child(diag))
                    .child(div().h(px(1.0)).w_full().bg(p.glass_line))
                    .child(
                        Label::new(tr.links)
                            .text_xs()
                            .text_color(p.muted_foreground),
                    )
                    .child(links_col)
                    .child(div().h(px(1.0)).w_full().bg(p.glass_line))
                    .child(
                        Label::new(format!(
                            "{} ({})",
                            tr.review_comments,
                            matching_comments.len()
                        ))
                        .text_xs()
                        .text_color(p.muted_foreground),
                    )
                    .child(target_bar)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.comment_input)))
                            .child(
                                Button::new("post-comment")
                                    .label(tr.post)
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.post_comment(window, cx);
                                    })),
                            ),
                    )
                    .when_some(self.error_message.clone(), |el, err| {
                        el.child(Label::new(err).text_xs().text_color(p.primary))
                    })
                    .child(comments_col),
            )
    }

    /// Live session preview + typed state editor matching `apps/canvas` LiveStage & LiveInspector.
    fn render_live_stage(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.pal();
        let tr = i18n::strings(self.lang);
        let Some(session) = self.live_session.as_ref() else {
            return div().id("live-stage-empty").flex_1().h_full();
        };
        let screen_path = self
            .active_screen()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| "/".into());

        let preview = if let Some(ri) = session.frame.clone() {
            let dims = ri.size(0);
            let w = (dims.width.0 as f32).min(960.0);
            let h = (dims.height.0 as f32).min(1200.0);
            div()
                .id("live-frame-scroll")
                .flex_1()
                .h_full()
                .overflow_y_scroll()
                .p_6()
                .flex()
                .items_start()
                .justify_center()
                .child(
                    div()
                        .p_2()
                        .rounded_xl()
                        .border_1()
                        .border_color(p.capture_rim)
                        .bg(p.capture_surface)
                        .shadow_lg()
                        .child(img(ImageSource::Render(ri)).w(px(w)).h(px(h))),
                )
        } else {
            div()
                .id("live-frame-missing")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new("Waiting for live frame…").text_color(p.muted_foreground))
        };

        let mut controls_col = div().flex().flex_col().gap_3();
        let editable_controls: Vec<_> = session.controls.iter().filter(|c| c.editable).collect();
        if editable_controls.is_empty() {
            controls_col = controls_col.child(
                Label::new(tr.no_editable_states)
                    .text_xs()
                    .text_color(p.muted_foreground),
            );
        } else {
            for (idx, ctrl) in editable_controls.iter().enumerate() {
                let inst = ctrl.instance.clone();
                let slot = ctrl.slot.clone();
                let mut opts_row = div().flex().flex_wrap().gap_1();
                for (o_idx, opt) in ctrl.options.iter().enumerate() {
                    let active = *opt == ctrl.value;
                    let opt_val = opt.clone();
                    let inst_c = inst.clone();
                    let slot_c = slot.clone();
                    let opt_label = match opt {
                        Value::Bool(true) => "On".to_owned(),
                        Value::Bool(false) => "Off".to_owned(),
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    opts_row = opts_row.child(
                        Button::new(format!("live-opt-{idx}-{o_idx}"))
                            .label(opt_label)
                            .when(active, |b| b.primary())
                            .when(!active, |b| b.outline())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(s) = this.live_session.as_mut()
                                    && let Err(e) = live_client::set_control(
                                        s,
                                        &inst_c,
                                        &slot_c,
                                        opt_val.clone(),
                                    )
                                {
                                    s.error = Some(e);
                                }
                                cx.notify();
                            })),
                    );
                }

                let changed = ctrl.value != ctrl.initial_value;
                controls_col = controls_col.child(
                    div()
                        .id(format!("live-ctrl-{idx}"))
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(p.glass_line)
                        .bg(p.glass_surface)
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    Label::new(format!("{} · {}", ctrl.owner, ctrl.label))
                                        .text_xs()
                                        .font_semibold()
                                        .text_color(p.foreground),
                                )
                                .when(changed, |el| {
                                    let inst_r = inst.clone();
                                    let slot_r = slot.clone();
                                    el.child(
                                        Button::new(format!("live-reset-{idx}"))
                                            .label(tr.reset)
                                            .ghost()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Some(s) = this.live_session.as_mut()
                                                    && let Err(e) = live_client::reset_control(
                                                        s, &inst_r, &slot_r,
                                                    )
                                                {
                                                    s.error = Some(e);
                                                }
                                                cx.notify();
                                            })),
                                    )
                                }),
                        )
                        .child(opts_row),
                );
            }
        }

        let live_inspector = div()
            .id("live-inspector-wrapper")
            .w(px(332.0))
            .h_full()
            .flex_shrink_0()
            .p_2()
            .child(
                div()
                    .id("live-inspector-panel")
                    .size_full()
                    .p_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(p.glass_line)
                    .bg(p.glass)
                    .shadow_lg()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        Label::new(route_short_name(&screen_path))
                                            .text_base()
                                            .font_bold()
                                            .text_color(p.foreground),
                                    )
                                    .child(
                                        Label::new(format!(
                                            "{} · {} px · rev {}",
                                            tr.live_controls, session.width_px, session.revision
                                        ))
                                        .text_xs()
                                        .text_color(p.muted_foreground),
                                    ),
                            )
                            .child(
                                div()
                                    .id("live-close-btn")
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .rounded_md()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.close_live_session(ViewMode::Detail, cx);
                                        }),
                                    )
                                    .child(
                                        Label::new("✕").text_sm().text_color(p.muted_foreground),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("live-save-snapshot-btn")
                            .w_full()
                            .py_2()
                            .rounded_md()
                            .bg(p.primary)
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    if let Some(s) = this.live_session.as_mut() {
                                        match live_client::save_session(s) {
                                            Ok(_) => {
                                                this.reload(cx);
                                            }
                                            Err(e) => {
                                                s.error = Some(e);
                                                cx.notify();
                                            }
                                        }
                                    }
                                }),
                            )
                            .child(
                                Label::new(if session.saved_run_id.is_some() {
                                    tr.saved_snapshot
                                } else {
                                    tr.save_snapshot
                                })
                                .text_xs()
                                .font_semibold()
                                .text_color(p.primary_foreground),
                            ),
                    )
                    .when_some(session.error.clone(), |el, err| {
                        el.child(Label::new(err).text_xs().text_color(p.primary))
                    })
                    .child(div().h(px(1.0)).w_full().bg(p.glass_line))
                    .child(controls_col),
            );

        div()
            .id("desktop-live-layout")
            .flex_1()
            .h_full()
            .flex()
            .flex_row()
            .child(preview)
            .child(live_inspector)
    }

    /// Capture Diff view matching `/tmp/obora-dui-diff.png` 1:1.
    fn render_diff_stage(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.pal();
        let tr = i18n::strings(self.lang);
        let Some(after_run) = self.active_run().cloned() else {
            return div()
                .id("desktop-diff-empty")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new(tr.workspace_empty).text_color(p.muted_foreground));
        };

        let peer_runs: Vec<(usize, String)> = self
            .workspace
            .runs
            .iter()
            .enumerate()
            .filter(|(i, r)| *i != self.selected_run && r.project_id == after_run.project_id)
            .map(|(i, r)| (i, format!("{} · {}", r.project_name, short_run_id(&r.id))))
            .collect();

        if peer_runs.is_empty() {
            return div()
                .id("desktop-diff-need-two")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new(tr.need_two_runs).text_color(p.muted_foreground));
        }

        self.ensure_diff_loaded();
        let entries = self.diff_entries.clone();

        let mut before_pills = div().flex().items_center().gap_1();
        for (idx, label) in peer_runs.into_iter().take(3) {
            let active = self.compare_run == Some(idx);
            before_pills = before_pills.child(
                Button::new(format!("diff-before-{idx}"))
                    .label(label)
                    .when(active, |b| b.primary())
                    .when(!active, |b| b.outline())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.compare_run = Some(idx);
                        this.diff_cache_key = None;
                        this.ensure_diff_loaded();
                        cx.notify();
                    })),
            );
        }

        let header = div()
            .mx_3()
            .mt_2()
            .px_4()
            .py_3()
            .rounded_xl()
            .border_1()
            .border_color(p.glass_line)
            .bg(p.glass)
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("diff-back-overview")
                            .label(format!("← {}", tr.back_to_overview))
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view_mode = ViewMode::Overview;
                                cx.notify();
                            })),
                    )
                    .child(
                        Label::new(tr.diff_title)
                            .text_sm()
                            .font_semibold()
                            .text_color(p.foreground),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                Label::new(tr.before_capture)
                                    .text_xs()
                                    .text_color(p.muted_foreground),
                            )
                            .child(before_pills),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                Label::new(tr.after_capture)
                                    .text_xs()
                                    .text_color(p.muted_foreground),
                            )
                            .child(
                                Label::new(format!(
                                    "{} · {}",
                                    after_run.project_name,
                                    short_run_id(&after_run.id)
                                ))
                                .text_xs()
                                .font_semibold()
                                .text_color(p.foreground),
                            ),
                    ),
            )
            .child(
                Label::new(format!("{} changed", entries.len()))
                    .text_xs()
                    .text_color(p.muted_foreground),
            );

        let mut cards = div()
            .id("desktop-diff-scroll")
            .flex_1()
            .overflow_y_scroll()
            .p_3()
            .flex()
            .flex_col()
            .gap_3();

        if entries.is_empty() {
            cards = cards.child(
                div()
                    .p_6()
                    .rounded_xl()
                    .border_1()
                    .border_color(p.glass_line)
                    .bg(p.glass)
                    .child(
                        Label::new(tr.no_changes)
                            .text_sm()
                            .text_color(p.muted_foreground),
                    ),
            );
        } else {
            for (idx, entry) in entries.iter().take(12).enumerate() {
                let before_img = entry
                    .before_blob
                    .as_deref()
                    .and_then(|path| self.images.get_or_decode(path));
                let after_img = entry
                    .after_blob
                    .as_deref()
                    .and_then(|path| self.images.get_or_decode(path));

                let meta = match (entry.changed_pixels, entry.diff_percent) {
                    (Some(px_cnt), Some(pct)) => format!(
                        "{} · {} px · {} · {px_cnt} px ({pct:.2}%)",
                        entry.path, entry.width_px, entry.kind
                    ),
                    _ => format!("{} · {} px · {}", entry.path, entry.width_px, entry.kind),
                };

                let thumb = |title: &str, ri: Option<Arc<RenderImage>>, accent_border: bool| {
                    let body = if let Some(image) = ri {
                        div()
                            .w(px(260.0))
                            .h(px(176.0))
                            .overflow_hidden()
                            .rounded_md()
                            .border_1()
                            .border_color(if accent_border { p.primary } else { p.border })
                            .bg(p.card)
                            .child(img(ImageSource::Render(image)).w(px(260.0)).h(px(176.0)))
                    } else {
                        div()
                            .w(px(260.0))
                            .h(px(176.0))
                            .rounded_md()
                            .border_1()
                            .border_color(p.glass_line)
                            .bg(p.card)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Label::new("—").text_xs().text_color(p.muted_foreground))
                    };
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            Label::new(title.to_owned())
                                .text_xs()
                                .text_color(p.muted_foreground),
                        )
                        .child(body)
                };

                cards = cards.child(
                    div()
                        .id(format!("diff-card-{idx}"))
                        .p_4()
                        .rounded_xl()
                        .border_1()
                        .border_color(p.glass_line)
                        .bg(p.glass)
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            Label::new(meta)
                                .text_xs()
                                .font_semibold()
                                .text_color(p.foreground),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_3()
                                .child(thumb(tr.before, before_img, false))
                                .child(thumb(tr.after, after_img, false))
                                .child(thumb(tr.overlay, entry.overlay.clone(), true)),
                        ),
                );
            }
        }

        div()
            .id("desktop-diff-stage")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(header)
            .child(cards)
    }

    /// Centered WorkPicker modal overlay matching `apps/canvas/src/features/workspace/WorkPicker.tsx`.
    fn render_work_picker_modal(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.pal();
        let tr = i18n::strings(self.lang);
        let groups = self.workspace.project_groups();

        let mut list = div()
            .id("work-picker-scroll")
            .max_h(px(420.0))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3();

        for group in groups {
            let mut col = div().flex().flex_col().gap_1().child(
                Label::new(format!(
                    "{} ({})",
                    group.project_name,
                    group.run_indices.len()
                ))
                .text_xs()
                .font_semibold()
                .text_color(p.muted_foreground),
            );
            for &run_idx in group.run_indices.iter().take(8) {
                let run = &self.workspace.runs[run_idx];
                let active = run_idx == self.selected_run;
                let (total, verified, _, _) = run.counts();
                let short = short_run_id(&run.id);
                let branch = run.branch.as_deref().unwrap_or("main");
                col = col.child(
                    div()
                        .id(format!("picker-run-{run_idx}"))
                        .w_full()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(if active { p.primary } else { p.glass_line })
                        .bg(p.glass_surface)
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_between()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.select_run(run_idx, cx);
                            }),
                        )
                        .child(
                            Label::new(format!("{short} · {branch}"))
                                .text_xs()
                                .font_semibold()
                                .text_color(p.foreground),
                        )
                        .child(
                            Label::new(format!(
                                "{} screens · {verified}/{total}",
                                run.screens.len()
                            ))
                            .text_xs()
                            .text_color(p.muted_foreground),
                        ),
                );
            }
            list = list.child(col);
        }

        div()
            .id("work-picker-backdrop")
            .absolute()
            .inset_0()
            .bg(p.scrim)
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.picker_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("work-picker-dialog")
                    .w(px(460.0))
                    .p_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(p.glass_line)
                    .bg(p.card)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                Label::new(tr.select_workspace)
                                    .text_sm()
                                    .font_semibold()
                                    .text_color(p.foreground),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("picker-open-diff")
                                            .label(tr.compare_captures)
                                            .outline()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.picker_open = false;
                                                this.view_mode = ViewMode::Diff;
                                                this.ensure_diff_loaded();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("picker-refresh")
                                            .label(tr.refresh)
                                            .ghost()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.reload(cx);
                                            })),
                                    ),
                            ),
                    )
                    .child(list),
            )
    }

    /// Map options dialog matching `apps/canvas` bottom-right `Map options` modal.
    fn render_map_options_modal(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.pal();
        let tr = i18n::strings(self.lang);
        let widths = self
            .active_run()
            .map(RunItem::widths)
            .unwrap_or_else(|| vec![1280]);

        let mut width_row = div().flex().flex_wrap().gap_2();
        for w in widths {
            let active = w == self.selected_width;
            width_row = width_row.child(
                Button::new(format!("opt-width-{w}"))
                    .label(format!("{w} px"))
                    .when(active, |b| b.primary())
                    .when(!active, |b| b.outline())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_width = w;
                        this.sync_cell_to_width();
                        this.options_open = false;
                        this.fit_map();
                        cx.notify();
                    })),
            );
        }

        div()
            .id("map-options-backdrop")
            .absolute()
            .inset_0()
            .bg(p.scrim)
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.options_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("map-options-dialog")
                    .w(px(340.0))
                    .p_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(p.glass_line)
                    .bg(p.card)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                Label::new(tr.map_options)
                                    .text_sm()
                                    .font_semibold()
                                    .text_color(p.foreground),
                            )
                            .child(Button::new("opt-close").label(tr.close).ghost().on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.options_open = false;
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(
                        Label::new(tr.widths)
                            .text_xs()
                            .text_color(p.muted_foreground),
                    )
                    .child(width_row)
                    .child(div().h(px(1.0)).w_full().bg(p.glass_line))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("opt-compare")
                                    .label(tr.compare_captures)
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.options_open = false;
                                        this.view_mode = ViewMode::Diff;
                                        this.ensure_diff_loaded();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("opt-refresh")
                                    .label(tr.refresh)
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.options_open = false;
                                        this.reload(cx);
                                    })),
                            ),
                    ),
            )
    }
}

impl Render for DuiDesktopView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal();
        let win_bounds = window.viewport_size();
        let rail_w = if self.rail_collapsed { 52.0 } else { 204.0 };
        let insp_w = if self.view_mode == ViewMode::Detail {
            332.0
        } else {
            0.0
        };
        self.viewport_w = (f32::from(win_bounds.width) - rail_w - insp_w).max(320.0);
        self.viewport_h = f32::from(win_bounds.height).max(320.0);

        let rail = self.render_workspace_rail(cx);
        let picker_modal = self.picker_open.then(|| self.render_work_picker_modal(cx));
        let options_modal = self.options_open.then(|| self.render_map_options_modal(cx));

        let mut root = div()
            .id("dui-desktop-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .relative()
            .size_full()
            .flex()
            .flex_row()
            .bg(p.background)
            .text_color(p.foreground)
            .child(rail);

        root = match self.view_mode {
            ViewMode::Overview => root.child(self.render_overview_stage(false, cx)),
            ViewMode::Detail => {
                if self.full_1x_detail {
                    root.child(self.render_detail_stage(cx))
                } else if let (Some(run), Some(screen)) =
                    (self.active_run().cloned(), self.active_screen().cloned())
                {
                    let map = self.render_overview_stage(true, cx);
                    let insp = self.render_floating_inspector(&run, &screen, cx);
                    root.child(map).child(insp)
                } else {
                    root.child(self.render_overview_stage(false, cx))
                }
            }
            ViewMode::Live => root.child(self.render_live_stage(cx)),
            ViewMode::Diff => root.child(self.render_diff_stage(cx)),
        };

        if let Some(modal) = picker_modal {
            root = root.child(modal);
        }
        if let Some(modal) = options_modal {
            root = root.child(modal);
        }
        root
    }
}

fn write_rgba_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create snapshot dir: {e}"))?;
    }
    let file = File::create(path).map_err(|e| format!("create png: {e}"))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("png header: {e}"))?;
    writer
        .write_image_data(rgba)
        .map_err(|e| format!("png write: {e}"))
}

fn collect_elements(window: &Window) -> Vec<Value> {
    window
        .elements()
        .into_iter()
        .filter_map(|el| {
            let label = el.label()?;
            let trimmed = label.trim();
            if trimmed.is_empty() {
                return None;
            }
            Some(json!({
                "role": el.role().map(|r| format!("{r:?}")),
                "label": trimmed,
            }))
        })
        .collect()
}

fn run_snapshot(opts: LaunchOptions, snapshot_out: PathBuf) -> Result<Value, String> {
    if let Err(err) = gpui_kit::platform::current_headless_renderer() {
        return Err(format!("offscreen renderer unavailable: {err}"));
    }

    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    let dark = opts.dark;
    cx.update(|app| {
        gpui_kit::init(app);
        Theme::change(
            if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            None,
            app,
        );
    });

    let live_base_url_out = opts.live_base_url.clone();
    let dui_home_str = opts.dui_home.display().to_string();
    let width = opts.viewport_width;
    let height = opts.viewport_height;
    let (handle, entity) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(width as f32), px(height as f32)),
                    })),
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| DuiDesktopView::new(opts, window, cx)),
            )
        })
        .map_err(|e| format!("open offscreen desktop window: {e}"))?;

    let elements = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            collect_elements(window)
        })
        .map_err(|e| format!("render offscreen frame: {e}"))?;

    let summary = cx.read_entity(&entity, |view, _| {
        let ws = &view.workspace;
        let total_screens: usize = ws.runs.iter().map(|r| r.screens.len()).sum();
        let total_cells: usize = ws
            .runs
            .iter()
            .flat_map(|r| r.screens.iter())
            .map(|s| s.cells.len())
            .sum();
        json!({
            "runs": ws.runs.len(),
            "screens": total_screens,
            "cells": total_cells,
            "comments": ws.comments.len(),
        })
    });

    let img = cx
        .capture_screenshot(handle)
        .map_err(|e| format!("capture desktop screenshot: {e}"))?;
    let (pixel_w, pixel_h) = img.dimensions();
    write_rgba_png(&snapshot_out, pixel_w, pixel_h, &img.into_raw())?;

    Ok(json!({
        "ok": true,
        "duiHome": dui_home_str,
        "liveBaseUrl": live_base_url_out,
        "snapshot": snapshot_out.display().to_string(),
        "pixelWidth": pixel_w,
        "pixelHeight": pixel_h,
        "summary": summary,
        "elements": elements,
        "renderer": if cfg!(target_os = "macos") { "metal" } else { "wgpu" },
    }))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut opts = LaunchOptions {
        dui_home: store::default_dui_home(),
        project: None,
        run_id: None,
        screen_path: None,
        live_base_url: env::var("DUI_LIVE_URL")
            .ok()
            .and_then(|v| validate_loopback_live_url(&v)),
        view: ViewMode::Overview,
        canvas_diff: false,
        dark: true,
        lang: UiLang::En,
        viewport_width: 1440,
        viewport_height: 900,
    };
    let mut snapshot_out: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--home" => {
                if let Some(val) = args.get(i + 1) {
                    opts.dui_home = PathBuf::from(val);
                    i += 1;
                }
            }
            "--project" => {
                if let Some(val) = args.get(i + 1) {
                    opts.project = Some(val.clone());
                    i += 1;
                }
            }
            "--run" => {
                if let Some(val) = args.get(i + 1) {
                    opts.run_id = Some(val.clone());
                    i += 1;
                }
            }
            "--screen" => {
                if let Some(val) = args.get(i + 1) {
                    opts.screen_path = Some(val.clone());
                    i += 1;
                }
            }
            "--live-url" => {
                if let Some(val) = args.get(i + 1) {
                    opts.live_base_url = validate_loopback_live_url(val);
                    i += 1;
                }
            }
            "--view" => {
                if let Some(val) = args.get(i + 1) {
                    opts.view = ViewMode::parse(val);
                    i += 1;
                }
            }
            "--canvas-diff" => {
                opts.canvas_diff = true;
            }
            "--theme" => {
                if let Some(val) = args.get(i + 1) {
                    opts.dark = !val.eq_ignore_ascii_case("light");
                    i += 1;
                }
            }
            "--lang" => {
                if let Some(val) = args.get(i + 1) {
                    opts.lang = UiLang::parse(val);
                    i += 1;
                }
            }
            "--snapshot-out" => {
                if let Some(val) = args.get(i + 1) {
                    snapshot_out = Some(PathBuf::from(val));
                    i += 1;
                }
            }
            "--width" => {
                if let Some(val) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                    opts.viewport_width = val;
                    i += 1;
                }
            }
            "--height" => {
                if let Some(val) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                    opts.viewport_height = val;
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(out_path) = snapshot_out {
        match run_snapshot(opts, out_path) {
            Ok(report) => {
                println!("{report}");
                return;
            }
            Err(err) => {
                eprintln!("{}", json!({"ok": false, "error": err}));
                std::process::exit(1);
            }
        }
    }

    let dark = opts.dark;
    let width = opts.viewport_width;
    let height = opts.viewport_height;
    gpui_kit::application().with_assets(Assets).run(move |cx| {
        gpui_kit::init(cx);
        Theme::change(
            if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            None,
            cx,
        );
        let bounds = Bounds::centered(None, size(px(width as f32), px(height as f32)), cx);
        let _ = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            move |window, cx| cx.new(|cx| DuiDesktopView::new(opts, window, cx)),
        );
        cx.activate(true);
    });
}
