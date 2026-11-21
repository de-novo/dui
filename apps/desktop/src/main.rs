mod i18n;
mod layout;
mod store;

use gpui_kit::{
    AppContext, Bounds, Context, Entity, HeadlessAppContext, Hsla, ImageSource, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels, Point, Render, RenderImage,
    ScrollDelta, ScrollEvent, SharedString, Window, WindowBounds, WindowOptions,
    assets::Assets,
    base::StyledExt,
    canvas,
    component::{
        ActiveTheme, Theme, ThemeMode,
        button::{Button, ButtonVariants as _},
        input::{Input, InputState},
        label::Label,
    },
    div, hsla, img, point,
    prelude::*,
    px, size,
    test::{TestQueryExt, TestWindowExt},
};
use i18n::UiLang;
use layout::{MapLayout, compute_layout};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
    sync::Arc,
};
use store::{
    CellItem, DesktopWorkspace, DiffCellEntry, ImageCache, NewCommentAnchor, RunItem, ScreenItem,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ViewMode {
    Overview,
    Detail,
    Diff,
}

impl ViewMode {
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "detail" => Self::Detail,
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

struct LaunchOptions {
    dui_home: PathBuf,
    project: Option<String>,
    run_id: Option<String>,
    view: ViewMode,
    dark: bool,
    lang: UiLang,
    viewport_width: u32,
    viewport_height: u32,
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
    view_mode: ViewMode,
    sidebar_open: bool,
    dark_mode: bool,
    lang: UiLang,
    // 2D infinite canvas camera
    pan_x: f32,
    pan_y: f32,
    zoom: f32,
    detail_zoom: f32,
    dragging: bool,
    drag_last: Point<Pixels>,
    viewport_w: f32,
    viewport_h: f32,
    // Comment composer
    comment_input: Entity<InputState>,
    comment_target: CommentTargetChoice,
    error_message: Option<String>,
    // Cached diff entries for current (before, after) pair
    diff_cache_key: Option<(usize, usize)>,
    diff_entries: Vec<DiffCellEntry>,
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
            // Prefer the most recent run that actually has captured images, else 0.
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

        let selected_width = workspace
            .runs
            .get(selected_run)
            .and_then(|r| {
                let ws = r.widths();
                ws.iter().copied().max()
            })
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
        let comment_input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));

        let mut this = Self {
            dui_home: opts.dui_home,
            workspace,
            images: ImageCache::default(),
            selected_run,
            compare_run,
            selected_screen: 0,
            selected_width,
            selected_cell: 0,
            view_mode: opts.view,
            sidebar_open: true,
            dark_mode: opts.dark,
            lang: opts.lang,
            pan_x: 36.0,
            pan_y: 36.0,
            zoom: 0.25,
            detail_zoom: 0.75,
            dragging: false,
            drag_last: point(px(0.0), px(0.0)),
            viewport_w: opts.viewport_width.saturating_sub(580).max(400) as f32,
            viewport_h: opts.viewport_height.saturating_sub(110).max(320) as f32,
            comment_input,
            comment_target: CommentTargetChoice::Screen,
            error_message: None,
            diff_cache_key: None,
            diff_entries: Vec::new(),
        };
        this.fit_map();
        if this.view_mode == ViewMode::Diff {
            this.ensure_diff_loaded();
        }
        this
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
            self.zoom = 0.25;
            self.pan_x = 32.0;
            self.pan_y = 32.0;
            return;
        }
        let pad = 48.0;
        let avail_w = (self.viewport_w - pad * 2.0).max(200.0);
        let avail_h = (self.viewport_h - pad * 2.0).max(200.0);
        let zx = avail_w / layout.content_width;
        let zy = avail_h / layout.content_height;
        self.zoom = zx.min(zy).clamp(0.05, 1.0);
        let scaled_w = layout.content_width * self.zoom;
        let scaled_h = layout.content_height * self.zoom;
        self.pan_x = ((self.viewport_w - scaled_w) * 0.5).max(pad * 0.5);
        self.pan_y = ((self.viewport_h - scaled_h) * 0.5).max(pad * 0.5);
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
            let cy_world = node.y + layout.metrics.card_height * 0.5;
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
        self.selected_screen = 0;
        self.selected_cell = 0;
        self.comment_target = CommentTargetChoice::Screen;
        if let Some(run) = self.workspace.runs.get(idx) {
            let widths = run.widths();
            if !widths.contains(&self.selected_width) {
                self.selected_width = widths.iter().copied().max().unwrap_or(1280);
            }
            self.compare_run = self
                .workspace
                .runs
                .iter()
                .enumerate()
                .find(|(i, r)| *i != idx && r.project_id == run.project_id)
                .map(|(i, _)| i);
        }
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
        let key = (before_idx, self.selected_run);
        if self.diff_cache_key == Some(key) {
            return;
        }
        if let (Some(before), Some(after)) = (
            self.workspace.runs.get(before_idx),
            self.workspace.runs.get(self.selected_run),
        ) {
            self.diff_entries = store::compute_run_diff(before, after);
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
                if let Some(run) = self.workspace.runs.get(self.selected_run) {
                    if self.selected_screen >= run.screens.len() {
                        self.selected_screen = 0;
                    }
                    if let Some(screen) = run.screens.get(self.selected_screen)
                        && self.selected_cell >= screen.cells.len()
                    {
                        self.selected_cell = 0;
                    }
                }
                self.diff_cache_key = None;
                if self.view_mode == ViewMode::Diff {
                    self.ensure_diff_loaded();
                }
                self.error_message = None;
            }
            Err(err) => {
                self.error_message = Some(err);
            }
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

    fn render_topbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tr = i18n::strings(self.lang);
        let widths = self
            .active_run()
            .map(RunItem::widths)
            .unwrap_or_else(|| vec![1280]);

        let mut width_pills = div().flex().items_center().gap_1();
        for w in widths {
            let active = w == self.selected_width;
            width_pills = width_pills.child(
                Button::new(format!("top-width-{w}"))
                    .label(format!("{w}px"))
                    .when(active, |b| b.primary())
                    .when(!active, |b| b.ghost())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_width = w;
                        this.fit_map();
                        cx.notify();
                    })),
            );
        }

        let audit_pill = if let Some(run) = self.active_run() {
            let (total, verified, _unverified, unrenderable) = run.counts();
            let renderable = total.saturating_sub(unrenderable);
            let pct = if total == 0 {
                0.0
            } else {
                (verified as f64 * 100.0) / total as f64
            };
            let open_comments: usize = self
                .workspace
                .comments
                .iter()
                .filter(|c| c.project_id == run.project_id && c.status == "open")
                .count();
            let meets_goal = total > 0 && unrenderable == 0 && pct >= 99.0;
            let dot_color = if meets_goal {
                hsla(142.0 / 360.0, 0.72, 0.44, 1.0)
            } else {
                hsla(24.0 / 360.0, 0.92, 0.54, 1.0)
            };
            let mut summary =
                format!("{pct:.1}% / 99% · {verified}/{total} ({renderable} renderable)");
            if open_comments > 0 {
                summary.push_str(&format!(" · {open_comments} open"));
            }
            div()
                .id("desktop-audit-pill")
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .rounded_full()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(dot_color))
                .child(Label::new(summary).text_xs().font_semibold())
        } else {
            div().id("desktop-audit-empty")
        };

        div()
            .id("desktop-topbar")
            .h(px(48.0))
            .w_full()
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("toggle-sidebar")
                            .label(if self.sidebar_open { "◀" } else { "▶" })
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.sidebar_open = !this.sidebar_open;
                                cx.notify();
                            })),
                    )
                    .child(Label::new(tr.app_title).font_semibold())
                    .child(
                        Button::new("mode-overview")
                            .label(tr.overview)
                            .when(self.view_mode == ViewMode::Overview, |b| b.primary())
                            .when(self.view_mode != ViewMode::Overview, |b| b.ghost())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view_mode = ViewMode::Overview;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("mode-detail")
                            .label(tr.detail)
                            .when(self.view_mode == ViewMode::Detail, |b| b.primary())
                            .when(self.view_mode != ViewMode::Detail, |b| b.ghost())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view_mode = ViewMode::Detail;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("mode-diff")
                            .label(tr.diff)
                            .when(self.view_mode == ViewMode::Diff, |b| b.primary())
                            .when(self.view_mode != ViewMode::Diff, |b| b.ghost())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view_mode = ViewMode::Diff;
                                this.ensure_diff_loaded();
                                cx.notify();
                            })),
                    )
                    .child(width_pills),
            )
            .child(audit_pill)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("toggle-theme")
                            .label(if self.dark_mode { tr.light } else { tr.dark })
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.dark_mode = !this.dark_mode;
                                let mode = if this.dark_mode {
                                    ThemeMode::Dark
                                } else {
                                    ThemeMode::Light
                                };
                                Theme::change(mode, None, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("toggle-lang")
                            .label(self.lang.short_label())
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.lang = this.lang.next();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("refresh-workspace")
                            .label(tr.refresh)
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reload(cx);
                            })),
                    ),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tr = i18n::strings(self.lang);
        let groups = self.workspace.project_groups();
        let mut list = div()
            .id("project-groups-scroll")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3();

        if groups.is_empty() {
            list = list.child(
                Label::new(tr.workspace_empty)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            );
        } else {
            for group in groups {
                let mut group_col = div().flex().flex_col().gap_1().child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_1()
                        .child(
                            Label::new(group.project_name.clone())
                                .text_xs()
                                .font_semibold(),
                        )
                        .child(
                            Label::new(format!("{} runs", group.run_indices.len()))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        ),
                );
                for &run_idx in group.run_indices.iter().take(6) {
                    let run = &self.workspace.runs[run_idx];
                    let active = run_idx == self.selected_run;
                    let (total, verified, _, _) = run.counts();
                    let short_id = run.id.split('-').next_back().unwrap_or(&run.id);
                    let branch = run.branch.as_deref().unwrap_or("main");
                    let label = format!(
                        "{short_id} · {branch} · {}sc ({verified}/{total})",
                        run.screens.len()
                    );
                    group_col = group_col.child(
                        Button::new(format!("run-item-{run_idx}"))
                            .label(label)
                            .when(active, |b| b.primary())
                            .when(!active, |b| b.ghost())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_run(run_idx, cx);
                            })),
                    );
                }
                list = list.child(group_col);
            }
        }

        div()
            .id("desktop-sidebar")
            .w(px(250.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(list)
    }

    fn render_overview_canvas(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tr = i18n::strings(self.lang);
        let Some(run) = self.active_run().cloned() else {
            return div()
                .id("desktop-stage-empty")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .child(Label::new(tr.workspace_empty));
        };

        let layout = compute_layout(&run.screens, self.selected_width);
        let open_by_path = self.workspace.open_comments_by_path(&run.project_id);

        // Pre-decode RenderImages synchronously so frame 1 displays crisp UI screenshots!
        let mut decoded_cards: BTreeMap<usize, Option<Arc<RenderImage>>> = BTreeMap::new();
        for (idx, screen) in run.screens.iter().enumerate() {
            let img_opt = self
                .cell_for_width(screen)
                .and_then(|c| c.blob_path.as_deref())
                .and_then(|p| self.images.get_or_decode(p));
            decoded_cards.insert(idx, img_opt);
        }

        let pan_x = self.pan_x;
        let pan_y = self.pan_y;
        let zoom = self.zoom;
        let metrics = layout.metrics;
        let card_w = (metrics.card_width * zoom).max(16.0);
        let frame_h = (metrics.frame_height * zoom).max(10.0);
        let caption_h = (metrics.caption_height * zoom).max(18.0);
        let card_h = frame_h + caption_h;

        let dot_color = if self.dark_mode {
            hsla(0.0, 0.0, 1.0, 0.12)
        } else {
            hsla(0.0, 0.0, 0.0, 0.12)
        };
        let verified_edge_color = hsla(24.0 / 360.0, 0.92, 0.54, 0.95);
        let extracted_edge_color = if self.dark_mode {
            hsla(0.0, 0.0, 0.72, 0.65)
        } else {
            hsla(0.0, 0.0, 0.38, 0.65)
        };

        // Snapshot node screen-space positions for GPU edge painting.
        let mut node_rects: BTreeMap<usize, (f32, f32)> = BTreeMap::new();
        for node in &layout.nodes {
            node_rects.insert(
                node.screen_idx,
                (pan_x + node.x * zoom, pan_y + node.y * zoom),
            );
        }
        let edges_for_paint = layout.edges.clone();
        let node_rects_for_paint = node_rects.clone();

        let background_and_edges = canvas(
            |_bounds, _window, _cx| {},
            move |bounds: Bounds<Pixels>, _, window: &mut Window, _cx| {
                // 1. Subtle Figma dot grid
                let spacing = (32.0 * zoom).clamp(16.0, 48.0);
                let ox = f32::from(bounds.origin.x);
                let oy = f32::from(bounds.origin.y);
                let bw = f32::from(bounds.size.width);
                let bh = f32::from(bounds.size.height);
                let start_x = ((pan_x % spacing) + spacing) % spacing;
                let start_y = ((pan_y % spacing) + spacing) % spacing;
                let mut gx = start_x;
                while gx < bw {
                    let mut gy = start_y;
                    while gy < bh {
                        let mut dot = PathBuilder::fill();
                        let p = point(px(ox + gx), px(oy + gy));
                        dot.add_polygon(
                            &[
                                p,
                                point(px(ox + gx + 1.5), px(oy + gy)),
                                point(px(ox + gx + 1.5), px(oy + gy + 1.5)),
                                point(px(ox + gx), px(oy + gy + 1.5)),
                            ],
                            true,
                        );
                        if let Ok(path) = dot.build() {
                            window.paint_path(path, dot_color);
                        }
                        gy += spacing;
                    }
                    gx += spacing;
                }

                // 2. GPU-painted Bezier transition edges with arrowheads
                let stroke_w = (2.2 * zoom).clamp(1.5, 3.5);
                for edge in &edges_for_paint {
                    let (Some(&(ax, ay)), Some(&(bx, by))) = (
                        node_rects_for_paint.get(&edge.from_idx),
                        node_rects_for_paint.get(&edge.to_idx),
                    ) else {
                        continue;
                    };
                    let (sx, sy, tx, ty) = if (bx - ax).abs() >= (by - ay).abs() {
                        if bx >= ax {
                            (ax + card_w, ay + card_h * 0.5, bx, by + card_h * 0.5)
                        } else {
                            (ax, ay + card_h * 0.5, bx + card_w, by + card_h * 0.5)
                        }
                    } else if by >= ay {
                        (ax + card_w * 0.5, ay + card_h, bx + card_w * 0.5, by)
                    } else {
                        (ax + card_w * 0.5, ay, bx + card_w * 0.5, by + card_h)
                    };

                    let p0 = point(px(ox + sx), px(oy + sy));
                    let p3 = point(px(ox + tx), px(oy + ty));
                    let dx = (tx - sx) * 0.45;
                    let c1 = point(px(ox + sx + dx), px(oy + sy));
                    let c2 = point(px(ox + tx - dx), px(oy + ty));

                    let is_verified = edge.source == "verified";
                    let is_inferred = edge.source == "inferred";
                    let mut pb = PathBuilder::stroke(px(stroke_w));
                    if is_inferred {
                        pb = pb.dash_array(&[px(3.0), px(4.0)]);
                    } else if !is_verified {
                        pb = pb.dash_array(&[px(7.0), px(4.0)]);
                    }
                    pb.move_to(p0);
                    pb.cubic_bezier_to(p3, c1, c2);
                    let color: Hsla = if is_verified {
                        verified_edge_color
                    } else {
                        extracted_edge_color
                    };
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, color);
                    }

                    // Arrowhead at target point
                    let angle = (ty - sy).atan2(tx - sx);
                    let head_len = 8.0_f32;
                    let left = point(
                        px(ox + tx - head_len * (angle - 0.45).cos()),
                        px(oy + ty - head_len * (angle - 0.45).sin()),
                    );
                    let right = point(
                        px(ox + tx - head_len * (angle + 0.45).cos()),
                        px(oy + ty - head_len * (angle + 0.45).sin()),
                    );
                    let mut arrow = PathBuilder::fill();
                    arrow.add_polygon(&[p3, left, right], true);
                    if let Ok(path) = arrow.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .size_full();

        let mut cards_layer = div().absolute().inset_0();
        let accent = hsla(24.0 / 360.0, 0.92, 0.54, 1.0);
        for node in &layout.nodes {
            let idx = node.screen_idx;
            let screen = &run.screens[idx];
            let (left, top) = node_rects[&idx];
            let selected = idx == self.selected_screen;
            let unverified = screen
                .cells
                .iter()
                .filter(|c| c.status_kind == "unverified")
                .count();
            let unrenderable = screen
                .cells
                .iter()
                .filter(|c| c.status_kind == "unrenderable")
                .count();
            let open_comments = open_by_path.get(&screen.path).copied().unwrap_or(0);
            let render_img = decoded_cards.get(&idx).and_then(|v| v.clone());

            let media_el = if let Some(ri) = render_img {
                div()
                    .w(px(card_w))
                    .h(px(frame_h))
                    .overflow_hidden()
                    .bg(cx.theme().secondary)
                    .child(img(ImageSource::Render(ri)).w(px(card_w)).h(px(frame_h)))
            } else {
                div()
                    .w(px(card_w))
                    .h(px(frame_h))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().secondary)
                    .child(
                        Label::new("No image")
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
            };

            let mut badges = div().flex().items_center().gap_1();
            if open_comments > 0 {
                badges = badges.child(
                    Label::new(format!("● {open_comments}"))
                        .text_xs()
                        .text_color(accent)
                        .font_semibold(),
                );
            }
            if unverified > 0 {
                badges = badges.child(
                    Label::new(format!("U:{unverified}"))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                );
            }
            if unrenderable > 0 {
                badges = badges.child(
                    Label::new(format!("X:{unrenderable}"))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                );
            }

            let card = div()
                .id(format!("map-card-{idx}"))
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(card_w))
                .h(px(card_h))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded_lg()
                .border_2()
                .border_color(if selected { accent } else { cx.theme().border })
                .bg(cx.theme().background)
                .shadow_md()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if this.selected_screen == idx && event.click_count >= 2 {
                            this.view_mode = ViewMode::Detail;
                        } else {
                            this.selected_screen = idx;
                            this.selected_cell = 0;
                            this.comment_target = CommentTargetChoice::Screen;
                        }
                        cx.notify();
                    }),
                )
                .child(media_el)
                .child(
                    div()
                        .h(px(caption_h))
                        .px_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_1()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .child(
                            Label::new(route_short_name(&screen.path))
                                .text_xs()
                                .font_semibold(),
                        )
                        .child(badges),
                );

            cards_layer = cards_layer.child(card);
        }

        // Floating bottom-left zoom controls matching `apps/canvas`
        let zoom_controls = div()
            .id("desktop-zoom-controls")
            .absolute()
            .left(px(16.0))
            .bottom(px(16.0))
            .flex()
            .items_center()
            .gap_1()
            .p_1()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .shadow_md()
            .child(
                Button::new("zoom-out")
                    .label("−")
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.adjust_zoom(0.8);
                        cx.notify();
                    })),
            )
            .child(
                Label::new(format!("{}%", (self.zoom * 100.0).round() as i32))
                    .text_xs()
                    .font_semibold(),
            )
            .child(
                Button::new("zoom-in")
                    .label("+")
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.adjust_zoom(1.25);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("zoom-1x")
                    .label("1:1")
                    .outline()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_one_to_one();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("zoom-fit")
                    .label(tr.fit)
                    .outline()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.fit_map();
                        cx.notify();
                    })),
            );

        div()
            .id("desktop-map-viewport")
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(cx.theme().background)
            .on_scroll(cx.listener(|this, event: &ScrollEvent, _, cx| {
                let delta = match event.delta {
                    ScrollDelta::Pixels(p) => (f32::from(p.x), f32::from(p.y)),
                    ScrollDelta::Lines(l) => (l.x * 24.0, l.y * 24.0),
                };
                if event.modifiers.platform || event.modifiers.control {
                    let factor = if delta.1 > 0.0 { 1.1 } else { 0.9 };
                    this.adjust_zoom(factor);
                } else {
                    // Figma-style two-finger free 2D trackpad pan!
                    this.pan_x += delta.0;
                    this.pan_y += delta.1;
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, _| {
                    this.dragging = true;
                    this.drag_last = event.position;
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.dragging {
                    let dx = f32::from(event.position.x) - f32::from(this.drag_last.x);
                    let dy = f32::from(event.position.y) - f32::from(this.drag_last.y);
                    this.pan_x += dx;
                    this.pan_y += dy;
                    this.drag_last = event.position;
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, _| {
                    this.dragging = false;
                }),
            )
            .child(background_and_edges)
            .child(cards_layer)
            .child(zoom_controls)
    }

    fn render_detail_stage(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tr = i18n::strings(self.lang);
        let Some(screen) = self.active_screen().cloned() else {
            return div()
                .id("desktop-detail-empty")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new(tr.workspace_empty));
        };
        let cell = self.active_cell().cloned();
        let render_img = cell
            .as_ref()
            .and_then(|c| c.blob_path.as_deref())
            .and_then(|p| self.images.get_or_decode(p));

        let status_header = match &cell {
            Some(c) => match c.reason.as_deref() {
                Some(r) => format!(
                    "{} · {} px · {} ({r})",
                    screen.path, c.width_px, c.status_kind
                ),
                None => format!("{} · {} px · {}", screen.path, c.width_px, c.status_kind),
            },
            None => screen.path.clone(),
        };

        let preview = if let Some(ri) = render_img {
            let size_px = ri.size(0);
            let w = (size_px.width.0 as f32 * self.detail_zoom).max(200.0);
            let h = (size_px.height.0 as f32 * self.detail_zoom).max(200.0);
            div()
                .id("detail-image-scroll")
                .flex_1()
                .overflow_y_scroll()
                .p_4()
                .flex()
                .justify_center()
                .child(
                    div()
                        .rounded_lg()
                        .overflow_hidden()
                        .border_1()
                        .border_color(cx.theme().border)
                        .shadow_lg()
                        .child(img(ImageSource::Render(ri)).w(px(w)).h(px(h))),
                )
        } else {
            div()
                .id("detail-image-empty")
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new("No PNG captured for this cell."))
        };

        div()
            .id("desktop-detail-stage")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(
                div()
                    .px_4()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("detail-back-map")
                                    .label(format!("← {}", tr.back_to_map))
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.view_mode = ViewMode::Overview;
                                        cx.notify();
                                    })),
                            )
                            .child(Label::new(status_header).text_sm().font_semibold()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(Button::new("detail-zoom-out").label("−").ghost().on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.detail_zoom = (this.detail_zoom * 0.8).clamp(0.25, 3.0);
                                    cx.notify();
                                }),
                            ))
                            .child(
                                Button::new("detail-zoom-1x")
                                    .label("1:1")
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.detail_zoom = 1.0;
                                        cx.notify();
                                    })),
                            )
                            .child(Button::new("detail-zoom-in").label("+").ghost().on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.detail_zoom = (this.detail_zoom * 1.25).clamp(0.25, 3.0);
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(preview)
    }

    fn render_diff_stage(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tr = i18n::strings(self.lang);
        let Some(after_run) = self.active_run().cloned() else {
            return div()
                .id("desktop-diff-empty")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new(tr.workspace_empty));
        };
        let peer_runs: Vec<(usize, String)> = self
            .workspace
            .runs
            .iter()
            .enumerate()
            .filter(|(i, r)| *i != self.selected_run && r.project_id == after_run.project_id)
            .map(|(i, r)| {
                let short = r.id.split('-').next_back().unwrap_or(&r.id);
                (i, format!("{short} ({}sc)", r.screens.len()))
            })
            .collect();

        if peer_runs.is_empty() {
            return div()
                .id("desktop-diff-need-two")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .child(Label::new(tr.need_two_runs));
        }

        self.ensure_diff_loaded();
        let mut selector_row = div().flex().items_center().gap_2().child(
            Label::new(format!("{}:", tr.before))
                .text_xs()
                .font_semibold(),
        );
        for (idx, label) in peer_runs.into_iter().take(5) {
            let active = self.compare_run == Some(idx);
            selector_row = selector_row.child(
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

        let entries = self.diff_entries.clone();
        let mut cards = div()
            .id("desktop-diff-scroll")
            .flex_1()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_4();

        if entries.is_empty() {
            cards = cards.child(Label::new(tr.no_changes).text_sm());
        } else {
            for (idx, entry) in entries.iter().take(12).enumerate() {
                let before_img = entry
                    .before_blob
                    .as_deref()
                    .and_then(|p| self.images.get_or_decode(p));
                let after_img = entry
                    .after_blob
                    .as_deref()
                    .and_then(|p| self.images.get_or_decode(p));
                let meta = match (entry.changed_pixels, entry.diff_percent) {
                    (Some(px_cnt), Some(pct)) => {
                        format!(
                            "{} · {} px · {} · {px_cnt} px ({pct:.2}%)",
                            entry.path, entry.width_px, entry.kind
                        )
                    }
                    _ => format!("{} · {} px · {}", entry.path, entry.width_px, entry.kind),
                };

                let thumb = |title: &str, ri: Option<Arc<RenderImage>>| {
                    let body = if let Some(image) = ri {
                        div()
                            .w(px(220.0))
                            .h(px(150.0))
                            .overflow_hidden()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(img(ImageSource::Render(image)).w(px(220.0)).h(px(150.0)))
                    } else {
                        div()
                            .w(px(220.0))
                            .h(px(150.0))
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Label::new("—").text_xs())
                    };
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            Label::new(title.to_owned())
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(body)
                };

                cards = cards.child(
                    div()
                        .id(format!("diff-card-{idx}"))
                        .p_3()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(Label::new(meta).text_sm().font_semibold())
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_3()
                                .child(thumb(tr.before, before_img))
                                .child(thumb(tr.after, after_img))
                                .child(thumb(tr.overlay, entry.overlay.clone())),
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
            .bg(cx.theme().background)
            .child(
                div()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary)
                    .child(selector_row),
            )
            .child(cards)
    }

    fn render_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tr = i18n::strings(self.lang);
        let Some(run) = self.active_run() else {
            return div().id("desktop-inspector-empty").w(px(320.0));
        };
        let Some(screen) = self.active_screen() else {
            return div().id("desktop-inspector-no-screen").w(px(320.0));
        };

        let mut cell_list = div().flex().flex_col().gap_1();
        for (c_idx, entry) in screen.cells.iter().enumerate() {
            let active = c_idx == self.selected_cell;
            let text = match (&entry.locale, &entry.reason) {
                (Some(loc), Some(r)) => {
                    format!("{}px · {loc} · {} ({r})", entry.width_px, entry.status_kind)
                }
                (Some(loc), None) => {
                    format!("{}px · {loc} · {}", entry.width_px, entry.status_kind)
                }
                (None, Some(r)) => format!("{}px · {} ({r})", entry.width_px, entry.status_kind),
                (None, None) => format!("{}px · {}", entry.width_px, entry.status_kind),
            };
            let entry_w = entry.width_px;
            cell_list = cell_list.child(
                Button::new(format!("insp-cell-{c_idx}"))
                    .label(text)
                    .when(active, |b| b.primary())
                    .when(!active, |b| b.outline())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_cell = c_idx;
                        this.selected_width = entry_w;
                        cx.notify();
                    })),
            );
        }

        let mut transitions_box = div().flex().flex_col().gap_1();
        if screen.outgoing.is_empty() {
            transitions_box = transitions_box.child(
                Label::new(tr.no_transitions)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            );
        } else {
            for (e_idx, edge) in screen.outgoing.iter().enumerate() {
                let target_idx = run.screens.iter().position(|s| s.path == edge.to_path);
                let label = format!("→ {} [{}]", edge.to_path, edge.source);
                if let Some(t_idx) = target_idx {
                    transitions_box = transitions_box.child(
                        Button::new(format!("insp-edge-{e_idx}"))
                            .label(label)
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selected_screen = t_idx;
                                this.selected_cell = 0;
                                this.comment_target = CommentTargetChoice::Screen;
                                cx.notify();
                            })),
                    );
                } else {
                    transitions_box = transitions_box.child(Label::new(label).text_xs());
                }
            }
        }

        // Comment target picker (Screen / Width / Transition)
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
            let is_active = self.comment_target == CommentTargetChoice::Transition(to.clone());
            target_bar = target_bar.child(
                Button::new("cmt-target-transition")
                    .label(format!("{} → {to}", tr.target_transition))
                    .when(is_active, |b| b.primary())
                    .when(!is_active, |b| b.outline())
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

        let mut comments_list = div()
            .id("comments-list-scroll")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        if matching_comments.is_empty() {
            comments_list = comments_list.child(
                Label::new(tr.no_comments)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            );
        } else {
            for (idx, c) in matching_comments.iter().enumerate() {
                let cid = c.id.clone();
                let is_open = c.status == "open";
                comments_list = comments_list.child(
                    div()
                        .id(format!("desktop-comment-{idx}"))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    Label::new(format!("{} · {}", c.target_label, c.status))
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
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
                        .child(Label::new(c.body.clone()).text_sm())
                        .child(
                            Label::new(c.author.clone())
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        ),
                );
            }
        }

        let open_count = matching_comments
            .iter()
            .filter(|c| c.status == "open")
            .count();

        div()
            .id("desktop-inspector")
            .w(px(330.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        Label::new(format!(
                            "{} · {}",
                            tr.inspector,
                            route_short_name(&screen.path)
                        ))
                        .font_semibold(),
                    )
                    .child(
                        Button::new("inspector-open-detail")
                            .label(tr.open_detail)
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view_mode = ViewMode::Detail;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                Label::new(screen.path.clone())
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(Label::new(tr.states_and_widths).text_xs().font_semibold())
            .child(cell_list)
            .child(Label::new(tr.transitions).text_xs().font_semibold())
            .child(transitions_box)
            .child(
                Label::new(format!(
                    "{} ({open_count}/{})",
                    tr.review_comments,
                    matching_comments.len()
                ))
                .text_xs()
                .font_semibold(),
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
                el.child(Label::new(err).text_xs())
            })
            .child(comments_list)
    }
}

impl Render for DuiDesktopView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let win_bounds = window.viewport_size();
        let sidebar_w = if self.sidebar_open { 250.0 } else { 0.0 };
        let inspector_w = if self.view_mode == ViewMode::Diff {
            0.0
        } else {
            330.0
        };
        self.viewport_w = (f32::from(win_bounds.width) - sidebar_w - inspector_w).max(320.0);
        self.viewport_h = (f32::from(win_bounds.height) - 48.0).max(260.0);

        let topbar = self.render_topbar(cx);
        let sidebar = self.sidebar_open.then(|| self.render_sidebar(cx));
        let inspector = (self.view_mode != ViewMode::Diff).then(|| self.render_inspector(cx));

        let mut body = div().flex_1().w_full().min_h_0().flex().flex_row();
        if let Some(sb) = sidebar {
            body = body.child(sb);
        }
        body = match self.view_mode {
            ViewMode::Overview => body.child(self.render_overview_canvas(cx)),
            ViewMode::Detail => body.child(self.render_detail_stage(cx)),
            ViewMode::Diff => body.child(self.render_diff_stage(cx)),
        };
        if let Some(insp) = inspector {
            body = body.child(insp);
        }

        div()
            .id("dui-desktop-root")
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(topbar)
            .child(body)
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
        view: ViewMode::Overview,
        dark: false,
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
            "--view" => {
                if let Some(val) = args.get(i + 1) {
                    opts.view = ViewMode::parse(val);
                    i += 1;
                }
            }
            "--theme" => {
                if let Some(val) = args.get(i + 1) {
                    opts.dark = val.eq_ignore_ascii_case("dark");
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
