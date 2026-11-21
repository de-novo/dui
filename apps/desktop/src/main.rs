mod store;

use gpui_kit::{
    AppContext, Bounds, Context, Entity, HeadlessAppContext, Image, ImageFormat, ImageSource,
    Render, SharedString, Window, WindowBounds, WindowOptions,
    assets::Assets,
    base::StyledExt,
    component::{
        ActiveTheme,
        button::{Button, ButtonVariants as _},
        input::{Input, InputState},
        label::Label,
    },
    div, img,
    prelude::*,
    px, size,
    test::{TestQueryExt, TestWindowExt},
};
use serde_json::{Value, json};
use std::{
    env,
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
    sync::Arc,
};
use store::{CellItem, DesktopWorkspace, RunItem, ScreenItem};

struct DuiDesktopView {
    dui_home: PathBuf,
    workspace: DesktopWorkspace,
    selected_run: usize,
    selected_screen: usize,
    selected_cell: usize,
    comment_input: Entity<InputState>,
    error_message: Option<String>,
}

impl DuiDesktopView {
    fn new(
        dui_home: PathBuf,
        initial_project: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let workspace = store::load_workspace(&dui_home).unwrap_or_else(|_| DesktopWorkspace {
            dui_home: dui_home.clone(),
            db_path: dui_home.join("dui.db"),
            runs: Vec::new(),
            comments: Vec::new(),
        });
        let selected_run = initial_project
            .and_then(|pid| {
                workspace
                    .runs
                    .iter()
                    .position(|r| r.project_id == pid || r.project_name == pid)
            })
            .unwrap_or(0);
        let comment_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Add a review comment to this screen...")
        });
        Self {
            dui_home,
            workspace,
            selected_run,
            selected_screen: 0,
            selected_cell: 0,
            comment_input,
            error_message: None,
        }
    }

    fn active_run(&self) -> Option<&RunItem> {
        self.workspace.runs.get(self.selected_run)
    }

    fn active_screen(&self) -> Option<&ScreenItem> {
        self.active_run()?.screens.get(self.selected_screen)
    }

    fn active_cell(&self) -> Option<&CellItem> {
        self.active_screen()?.cells.get(self.selected_cell)
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
        match store::add_screen_comment(
            &self.dui_home,
            &project_id,
            &run_id,
            &screen_path,
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

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut run_list = div()
            .id("run-list-scroll")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        if self.workspace.runs.is_empty() {
            run_list = run_list.child(
                Label::new("No captures in ~/.dui/dui.db yet.")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            );
        } else {
            for (idx, run) in self.workspace.runs.iter().enumerate() {
                let active = idx == self.selected_run;
                let branch = run.branch.as_deref().unwrap_or("detached");
                let label = format!("{} · {} ({})", run.project_name, branch, run.screens.len());
                run_list = run_list.child(
                    Button::new(format!("run-item-{idx}"))
                        .label(label)
                        .when(active, |b| b.primary())
                        .when(!active, |b| b.ghost())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected_run = idx;
                            this.selected_screen = 0;
                            this.selected_cell = 0;
                            cx.notify();
                        })),
                );
            }
        }

        div()
            .id("desktop-sidebar")
            .w(px(260.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Label::new("dui desktop").font_semibold())
                    .child(
                        Button::new("refresh-workspace")
                            .label("Refresh")
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reload(cx);
                            })),
                    ),
            )
            .child(
                Label::new(format!(
                    "SQLite runs ({}) · comments ({})",
                    self.workspace.runs.len(),
                    self.workspace.comments.len()
                ))
                .text_xs()
                .text_color(cx.theme().muted_foreground),
            )
            .child(run_list)
    }

    fn render_stage(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(run) = self.active_run() else {
            return div()
                .id("desktop-stage-empty")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .child(Label::new(
                    "Run `dui map <project>` or `dui run <project> <manifest>` to populate ~/.dui/dui.db.",
                ));
        };

        let mut screen_cards = div().flex().flex_wrap().gap_2();
        for (idx, screen) in run.screens.iter().enumerate() {
            let active = idx == self.selected_screen;
            let verified = screen
                .cells
                .iter()
                .filter(|c| c.status_kind == "verified")
                .count();
            let total = screen.cells.len();
            let title = format!("{} ({verified}/{total})", screen.path);
            screen_cards = screen_cards.child(
                Button::new(format!("screen-node-{idx}"))
                    .label(title)
                    .when(active, |b| b.primary())
                    .when(!active, |b| b.outline())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_screen = idx;
                        this.selected_cell = 0;
                        cx.notify();
                    })),
            );
        }

        let preview_panel = match (self.active_screen(), self.active_cell()) {
            (Some(screen), Some(cell)) => {
                let mut width_bar = div().flex().flex_wrap().gap_1();
                for (c_idx, entry) in screen.cells.iter().enumerate() {
                    let active = c_idx == self.selected_cell;
                    let text = match entry.locale.as_deref() {
                        Some(loc) => {
                            format!("{} px · {loc} · {}", entry.width_px, entry.status_kind)
                        }
                        None => format!("{} px · {}", entry.width_px, entry.status_kind),
                    };
                    width_bar = width_bar.child(
                        Button::new(format!("cell-width-{c_idx}"))
                            .label(text)
                            .when(active, |b| b.primary())
                            .when(!active, |b| b.outline())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selected_cell = c_idx;
                                cx.notify();
                            })),
                    );
                }

                let status_line = match cell.reason.as_deref() {
                    Some(reason) => format!(
                        "{} · {} px · {} ({reason})",
                        screen.path, cell.width_px, cell.status_kind
                    ),
                    None => format!(
                        "{} · {} px · {}",
                        screen.path, cell.width_px, cell.status_kind
                    ),
                };

                let media = if let Some(blob_path) = cell.blob_path.as_ref()
                    && let Ok(bytes) = fs::read(blob_path)
                {
                    let image = Arc::new(Image::from_bytes(ImageFormat::Png, bytes));
                    div()
                        .id("capture-preview-frame")
                        .flex_1()
                        .overflow_hidden()
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            img(ImageSource::Image(image))
                                .max_w(px(620.0))
                                .max_h(px(480.0)),
                        )
                } else {
                    div()
                        .id("capture-preview-missing")
                        .flex_1()
                        .p_6()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Label::new(format!(
                            "No PNG blob stored for {} ({})",
                            screen.path, cell.status_kind
                        )))
                };

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new(status_line).font_semibold())
                    .child(width_bar)
                    .child(media)
            }
            _ => div().child(Label::new("Select a screen node to inspect captures.")),
        };

        div()
            .id("desktop-stage")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(cx.theme().background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        Label::new(format!(
                            "Screen map · {} ({} screens)",
                            run.project_name,
                            run.screens.len()
                        ))
                        .font_semibold(),
                    )
                    .child(
                        Label::new(format!("Run {}", run.id))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(screen_cards)
            .child(preview_panel)
    }

    fn render_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(run) = self.active_run() else {
            return div().id("desktop-inspector-empty").w(px(320.0));
        };
        let Some(screen) = self.active_screen() else {
            return div().id("desktop-inspector-no-screen").w(px(320.0));
        };

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

        let mut transitions_box = div().flex().flex_col().gap_1();
        if screen.outgoing.is_empty() {
            transitions_box = transitions_box.child(
                Label::new("No outgoing transitions recorded.")
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            );
        } else {
            for edge in &screen.outgoing {
                transitions_box = transitions_box.child(
                    Label::new(format!(
                        "{} -> {} [{}]",
                        edge.from_path, edge.to_path, edge.source
                    ))
                    .text_xs(),
                );
            }
        }

        let mut comments_list = div()
            .id("comments-list-scroll")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        if matching_comments.is_empty() {
            comments_list = comments_list.child(
                Label::new("No review comments on this screen yet.")
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
                                        .label(if is_open { "Resolve" } else { "Reopen" })
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
            .w(px(320.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(Label::new(format!("Inspector · {}", screen.path)).font_semibold())
            .child(Label::new("Transitions").text_xs().font_semibold())
            .child(transitions_box)
            .child(
                Label::new(format!(
                    "Review comments ({open_count}/{} open)",
                    matching_comments.len()
                ))
                .text_xs()
                .font_semibold(),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.comment_input)))
                    .child(
                        Button::new("post-comment")
                            .label("Post")
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("dui-desktop-root")
            .size_full()
            .flex()
            .flex_row()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(cx))
            .child(self.render_stage(cx))
            .child(self.render_inspector(cx))
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

fn run_snapshot(
    dui_home: PathBuf,
    project: Option<String>,
    snapshot_out: PathBuf,
    width: u32,
    height: u32,
) -> Result<Value, String> {
    if let Err(err) = gpui_kit::platform::current_headless_renderer() {
        return Err(format!("offscreen renderer unavailable: {err}"));
    }

    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let home_clone = dui_home.clone();
    let project_clone = project.clone();
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
                |window, cx| {
                    cx.new(|cx| {
                        DuiDesktopView::new(home_clone, project_clone.as_deref(), window, cx)
                    })
                },
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
        "duiHome": dui_home.display().to_string(),
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
    let mut dui_home = store::default_dui_home();
    let mut project: Option<String> = None;
    let mut snapshot_out: Option<PathBuf> = None;
    let mut width: u32 = 1280;
    let mut height: u32 = 800;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--home" => {
                if let Some(val) = args.get(i + 1) {
                    dui_home = PathBuf::from(val);
                    i += 1;
                }
            }
            "--project" => {
                if let Some(val) = args.get(i + 1) {
                    project = Some(val.clone());
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
                    width = val;
                    i += 1;
                }
            }
            "--height" => {
                if let Some(val) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                    height = val;
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(out_path) = snapshot_out {
        match run_snapshot(dui_home, project, out_path, width, height) {
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

    gpui_kit::application().with_assets(Assets).run(move |cx| {
        gpui_kit::init(cx);
        let bounds = Bounds::centered(None, size(px(width as f32), px(height as f32)), cx);
        let home_for_win = dui_home.clone();
        let project_for_win = project.clone();
        let _ = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            move |window, cx| {
                cx.new(|cx| {
                    DuiDesktopView::new(
                        home_for_win.clone(),
                        project_for_win.as_deref(),
                        window,
                        cx,
                    )
                })
            },
        );
        cx.activate(true);
    });
}
