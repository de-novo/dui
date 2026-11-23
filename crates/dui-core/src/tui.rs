use crate::diff::{BoxModel, BoxSpacing, RenderNodeSnapshot, RenderRect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_FG: [u8; 3] = [244, 244, 245];
pub const DEFAULT_BG: [u8; 3] = [24, 24, 27];

const ANSI_16: [[u8; 3]; 16] = [
    [24, 24, 27],    // 0 black
    [239, 68, 68],   // 1 red
    [34, 197, 94],   // 2 green
    [234, 179, 8],   // 3 yellow
    [59, 130, 246],  // 4 blue
    [168, 85, 247],  // 5 magenta
    [6, 182, 212],   // 6 cyan
    [228, 228, 231], // 7 white
    [113, 113, 122], // 8 bright black
    [248, 113, 113], // 9 bright red
    [74, 222, 128],  // 10 bright green
    [250, 204, 21],  // 11 bright yellow
    [96, 165, 250],  // 12 bright blue
    [192, 132, 252], // 13 bright magenta
    [34, 211, 238],  // 14 bright cyan
    [255, 255, 255], // 15 bright white
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TerminalCell {
    pub ch: char,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub bold: bool,
    pub dim: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            bold: false,
            dim: false,
            underline: false,
            inverse: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalGrid {
    cols: usize,
    rows: usize,
    cells: Vec<TerminalCell>,
    cursor_row: usize,
    cursor_col: usize,
    fg: [u8; 3],
    bg: [u8; 3],
    bold: bool,
    dim: bool,
    underline: bool,
    inverse: bool,
    last_char: Option<char>,
}

impl TerminalGrid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        Self {
            cols,
            rows,
            cells: vec![TerminalCell::default(); cols * rows],
            cursor_row: 0,
            cursor_col: 0,
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            bold: false,
            dim: false,
            underline: false,
            inverse: false,
            last_char: None,
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cell(&self, row: usize, col: usize) -> TerminalCell {
        if row < self.rows && col < self.cols {
            self.cells[row * self.cols + col]
        } else {
            TerminalCell::default()
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            match ch {
                '\x1b' => {
                    i += 1;
                    if i >= chars.len() {
                        break;
                    }
                    match chars[i] {
                        '[' => {
                            i += 1;
                            let start = i;
                            while i < chars.len() && !('@'..='~').contains(&chars[i]) {
                                i += 1;
                            }
                            if i < chars.len() {
                                let params: String = chars[start..i].iter().collect();
                                let final_ch = chars[i];
                                self.apply_csi(&params, final_ch);
                                i += 1;
                            }
                        }
                        ']' => {
                            // OSC sequence terminated by BEL (\x07) or ST (\x1b\\)
                            i += 1;
                            while i < chars.len() {
                                if chars[i] == '\x07' {
                                    i += 1;
                                    break;
                                }
                                if chars[i] == '\x1b' && i + 1 < chars.len() && chars[i + 1] == '\\'
                                {
                                    i += 2;
                                    break;
                                }
                                i += 1;
                            }
                        }
                        '(' | ')' | '*' | '+' => {
                            i = (i + 2).min(chars.len());
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }
                '\r' => {
                    self.cursor_col = 0;
                    i += 1;
                }
                '\n' => {
                    self.line_feed();
                    i += 1;
                }
                '\x08' => {
                    self.cursor_col = self.cursor_col.saturating_sub(1);
                    i += 1;
                }
                '\t' => {
                    let next_tab = ((self.cursor_col / 8) + 1) * 8;
                    self.cursor_col = next_tab.min(self.cols.saturating_sub(1));
                    i += 1;
                }
                c if c.is_control() => {
                    i += 1;
                }
                c => {
                    self.put_char(c);
                    i += 1;
                }
            }
        }
    }

    fn put_char(&mut self, ch: char) {
        self.last_char = Some(ch);
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.line_feed();
        }
        let idx = self.cursor_row * self.cols + self.cursor_col;
        if let Some(cell) = self.cells.get_mut(idx) {
            *cell = TerminalCell {
                ch,
                fg: self.fg,
                bg: self.bg,
                bold: self.bold,
                dim: self.dim,
                underline: self.underline,
                inverse: self.inverse,
            };
        }
        self.cursor_col += 1;
    }

    fn line_feed(&mut self) {
        if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
        } else {
            // Scroll up one row
            let row_len = self.cols;
            self.cells.copy_within(row_len.., 0);
            let start = (self.rows - 1) * row_len;
            for cell in &mut self.cells[start..] {
                *cell = TerminalCell::default();
            }
        }
    }

    fn clear_all(&mut self) {
        for cell in &mut self.cells {
            *cell = TerminalCell::default();
        }
    }

    fn apply_csi(&mut self, raw_params: &str, final_ch: char) {
        if let Some(private) = raw_params.strip_prefix('?') {
            if (private == "1049" || private == "47" || private == "1047")
                && (final_ch == 'h' || final_ch == 'l')
            {
                self.clear_all();
                self.cursor_row = 0;
                self.cursor_col = 0;
            }
            return;
        }
        let nums: Vec<usize> = if raw_params.is_empty() {
            Vec::new()
        } else {
            raw_params
                .split(';')
                .map(|part| part.parse::<usize>().unwrap_or(0))
                .collect()
        };
        let p0 = nums.first().copied().unwrap_or(0);
        match final_ch {
            'A' => {
                let n = p0.max(1);
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            'B' => {
                let n = p0.max(1);
                self.cursor_row = (self.cursor_row + n).min(self.rows - 1);
            }
            'C' => {
                let n = p0.max(1);
                self.cursor_col = (self.cursor_col + n).min(self.cols - 1);
            }
            'D' => {
                let n = p0.max(1);
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            'G' => {
                let col = p0.saturating_sub(1);
                self.cursor_col = col.min(self.cols - 1);
            }
            'd' => {
                let row = p0.saturating_sub(1);
                self.cursor_row = row.min(self.rows - 1);
            }
            'H' | 'f' => {
                let row = nums.first().copied().unwrap_or(1).saturating_sub(1);
                let col = nums.get(1).copied().unwrap_or(1).saturating_sub(1);
                self.cursor_row = row.min(self.rows - 1);
                self.cursor_col = col.min(self.cols - 1);
            }
            'J' => match p0 {
                2 | 3 => self.clear_all(),
                0 => {
                    let start = self.cursor_row * self.cols + self.cursor_col.min(self.cols);
                    for cell in self.cells.iter_mut().skip(start) {
                        *cell = TerminalCell::default();
                    }
                }
                1 => {
                    let end =
                        (self.cursor_row * self.cols + self.cursor_col + 1).min(self.cells.len());
                    for cell in &mut self.cells[..end] {
                        *cell = TerminalCell::default();
                    }
                }
                _ => {}
            },
            'K' => {
                let row_start = self.cursor_row * self.cols;
                let row_end = row_start + self.cols;
                let cur = (row_start + self.cursor_col).min(row_end);
                match p0 {
                    0 => {
                        for cell in &mut self.cells[cur..row_end] {
                            *cell = TerminalCell::default();
                        }
                    }
                    1 => {
                        for cell in &mut self.cells[row_start..=cur.min(row_end - 1)] {
                            *cell = TerminalCell::default();
                        }
                    }
                    2 => {
                        for cell in &mut self.cells[row_start..row_end] {
                            *cell = TerminalCell::default();
                        }
                    }
                    _ => {}
                }
            }
            'X' => {
                // ECH: erase next N characters on the current line using current background.
                let n = p0.max(1);
                let row_start = self.cursor_row * self.cols;
                let row_end = row_start + self.cols;
                let cur = (row_start + self.cursor_col).min(row_end);
                let end = (cur + n).min(row_end);
                for cell in &mut self.cells[cur..end] {
                    *cell = TerminalCell {
                        bg: self.bg,
                        ..TerminalCell::default()
                    };
                }
            }
            'b' => {
                // REP: repeat the preceding graphic character N times (used heavily by ncurses/htop).
                if let Some(ch) = self.last_char {
                    let n = p0.max(1).min(self.cols * self.rows);
                    for _ in 0..n {
                        self.put_char(ch);
                    }
                }
            }
            'm' => {
                if nums.is_empty() {
                    self.reset_sgr();
                } else {
                    self.apply_sgr(&nums);
                }
            }
            _ => {}
        }
    }

    fn reset_sgr(&mut self) {
        self.fg = DEFAULT_FG;
        self.bg = DEFAULT_BG;
        self.bold = false;
        self.dim = false;
        self.underline = false;
        self.inverse = false;
    }

    fn apply_sgr(&mut self, nums: &[usize]) {
        let mut i = 0;
        while i < nums.len() {
            match nums[i] {
                0 => self.reset_sgr(),
                1 => self.bold = true,
                2 => self.dim = true,
                4 => self.underline = true,
                7 => self.inverse = true,
                22 => {
                    self.bold = false;
                    self.dim = false;
                }
                24 => self.underline = false,
                27 => self.inverse = false,
                30..=37 => self.fg = ANSI_16[nums[i] - 30],
                39 => self.fg = DEFAULT_FG,
                40..=47 => self.bg = ANSI_16[nums[i] - 40],
                49 => self.bg = DEFAULT_BG,
                90..=97 => self.fg = ANSI_16[nums[i] - 90 + 8],
                100..=107 => self.bg = ANSI_16[nums[i] - 100 + 8],
                38 | 48 => {
                    let is_fg = nums[i] == 38;
                    if i + 2 < nums.len() && nums[i + 1] == 5 {
                        let color = xterm_256_color(nums[i + 2] as u8);
                        if is_fg {
                            self.fg = color;
                        } else {
                            self.bg = color;
                        }
                        i += 2;
                    } else if i + 4 < nums.len() && nums[i + 1] == 2 {
                        let color = [
                            nums[i + 2].min(255) as u8,
                            nums[i + 3].min(255) as u8,
                            nums[i + 4].min(255) as u8,
                        ];
                        if is_fg {
                            self.fg = color;
                        } else {
                            self.bg = color;
                        }
                        i += 4;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    pub fn plain_text(&self) -> String {
        let mut lines: Vec<String> = (0..self.rows)
            .map(|r| {
                let start = r * self.cols;
                let line: String = self.cells[start..start + self.cols]
                    .iter()
                    .map(|c| c.ch)
                    .collect();
                line.trim_end().to_owned()
            })
            .collect();
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines.join("\n")
    }

    pub fn to_render_tree(
        &self,
        cell_w: i32,
        cell_h: i32,
        pad_x: i32,
        pad_y: i32,
    ) -> Vec<RenderNodeSnapshot> {
        let mut nodes = Vec::new();
        for r in 0..self.rows {
            let row_slice = &self.cells[r * self.cols..(r + 1) * self.cols];
            let first = row_slice
                .iter()
                .position(|c| c.ch != ' ' || c.bg != DEFAULT_BG);
            let last = row_slice
                .iter()
                .rposition(|c| c.ch != ' ' || c.bg != DEFAULT_BG);
            let (Some(start_col), Some(end_col)) = (first, last) else {
                continue;
            };
            let text: String = row_slice[start_col..=end_col]
                .iter()
                .map(|c| c.ch)
                .collect::<String>()
                .trim_end()
                .to_owned();
            let sample = row_slice[start_col..=end_col]
                .iter()
                .find(|c| c.ch.is_alphanumeric())
                .or_else(|| {
                    row_slice[start_col..=end_col]
                        .iter()
                        .find(|c| c.ch != ' ' && !is_box_drawing_char(c.ch))
                })
                .or_else(|| row_slice[start_col..=end_col].iter().find(|c| c.ch != ' '))
                .copied()
                .unwrap_or(row_slice[start_col]);
            let (fg, bg) = if sample.inverse {
                (sample.bg, sample.fg)
            } else {
                (sample.fg, sample.bg)
            };
            let span_cols = (end_col - start_col + 1) as i32;
            let mut style = BTreeMap::new();
            style.insert(
                "color".into(),
                format!("rgb({}, {}, {})", fg[0], fg[1], fg[2]),
            );
            style.insert(
                "backgroundColor".into(),
                format!("rgb({}, {}, {})", bg[0], bg[1], bg[2]),
            );
            style.insert(
                "fontWeight".into(),
                if sample.bold { "700" } else { "400" }.into(),
            );
            let mut attributes = BTreeMap::new();
            attributes.insert("row".into(), r.to_string());
            attributes.insert("colStart".into(), start_col.to_string());
            attributes.insert("colEnd".into(), end_col.to_string());
            attributes.insert("cols".into(), self.cols.to_string());
            attributes.insert("rows".into(), self.rows.to_string());
            nodes.push(RenderNodeSnapshot {
                id: format!("tui-row#{r}"),
                tag: "tui-row".into(),
                role: Some("row".into()),
                text,
                rect: RenderRect {
                    x: pad_x + (start_col as i32) * cell_w,
                    y: pad_y + (r as i32) * cell_h,
                    width: span_cols * cell_w,
                    height: cell_h,
                },
                style,
                parent_id: Some("terminal#0".into()),
                box_model: Some(BoxModel {
                    margin: BoxSpacing::default(),
                    padding: BoxSpacing::default(),
                    border: BoxSpacing::default(),
                    border_radius: None,
                    gap: None,
                }),
                attributes,
            });
        }
        nodes
    }
}

fn is_box_drawing_char(ch: char) -> bool {
    matches!(ch, '\u{2500}'..='\u{259F}')
}

fn xterm_256_color(n: u8) -> [u8; 3] {
    if n < 16 {
        return ANSI_16[n as usize];
    }
    if n >= 232 {
        let gray = 8 + (n - 232) * 10;
        return [gray, gray, gray];
    }
    let idx = n - 16;
    let r = idx / 36;
    let g = (idx % 36) / 6;
    let b = idx % 6;
    let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
    [level(r), level(g), level(b)]
}

pub fn encode_key_token(token: &str) -> Vec<u8> {
    match token {
        "Enter" | "Return" => b"\r".to_vec(),
        "Tab" => b"\t".to_vec(),
        "BackTab" | "Shift+Tab" => b"\x1b[Z".to_vec(),
        "Esc" | "Escape" => b"\x1b".to_vec(),
        "Up" | "ArrowUp" => b"\x1b[A".to_vec(),
        "Down" | "ArrowDown" => b"\x1b[B".to_vec(),
        "Right" | "ArrowRight" => b"\x1b[C".to_vec(),
        "Left" | "ArrowLeft" => b"\x1b[D".to_vec(),
        "Backspace" => vec![0x7f],
        "Space" => b" ".to_vec(),
        "Ctrl+C" => vec![0x03],
        "Ctrl+D" => vec![0x04],
        "Ctrl+L" => vec![0x0c],
        other => other.as_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff_render_trees;

    #[test]
    fn vt100_grid_parses_cursor_positioning_colors_and_converts_to_render_tree() {
        let mut grid = TerminalGrid::new(40, 10);
        grid.feed(b"\x1b[2J\x1b[1;1H\x1b[1;32mDashboard\x1b[0m\x1b[3;5HStatus: OK");
        assert_eq!(grid.plain_text(), "Dashboard\n\n    Status: OK");

        let tree = grid.to_render_tree(9, 18, 16, 16);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].id, "tui-row#0");
        assert_eq!(tree[0].text, "Dashboard");
        assert_eq!(tree[0].rect.x, 16);
        assert_eq!(tree[0].rect.y, 16);
        assert_eq!(
            tree[0].style.get("fontWeight").map(String::as_str),
            Some("700")
        );
        assert_eq!(
            tree[0].style.get("color").map(String::as_str),
            Some("rgb(34, 197, 94)")
        );

        assert_eq!(tree[1].id, "tui-row#2");
        assert_eq!(tree[1].text, "Status: OK");
        assert_eq!(tree[1].rect.x, 16 + 4 * 9);
        assert_eq!(tree[1].rect.y, 16 + 2 * 18);
    }

    #[test]
    fn tui_render_trees_diff_cleanly_through_shared_diff_render_trees() {
        let mut before_grid = TerminalGrid::new(40, 6);
        before_grid.feed(b"\x1b[1;1HOrders: 12\x1b[2;1HReady");
        let mut after_grid = TerminalGrid::new(40, 6);
        after_grid.feed(b"\x1b[1;1HOrders: 42\x1b[2;1HReady");

        let diff = diff_render_trees(
            &before_grid.to_render_tree(9, 18, 16, 16),
            &after_grid.to_render_tree(9, 18, 16, 16),
            true,
        );
        assert_eq!(diff.text_changed, 1);
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].id, "tui-row#0");
        assert_eq!(diff.changes[0].before_text.as_deref(), Some("Orders: 12"));
        assert_eq!(diff.changes[0].after_text.as_deref(), Some("Orders: 42"));
    }

    #[test]
    fn encodes_standard_tui_key_tokens() {
        assert_eq!(encode_key_token("Tab"), b"\t");
        assert_eq!(encode_key_token("Enter"), b"\r");
        assert_eq!(encode_key_token("Up"), b"\x1b[A");
        assert_eq!(encode_key_token("Ctrl+C"), vec![0x03]);
        assert_eq!(encode_key_token("q"), b"q");
    }

    #[test]
    fn handles_rep_ech_and_samples_alphanumeric_cell_color_over_dim_border() {
        let mut grid = TerminalGrid::new(30, 4);
        // Repeat '-' 5 times via CSI 5 b, then write text and erase 2 chars via CSI 2 X.
        grid.feed(b"\x1b[1;1H-\x1b[5b OK\x1b[1;2H\x1b[2X");
        assert_eq!(grid.plain_text(), "-  --- OK");

        // Row 2 has a dim gray box border followed by bright white text; style must sample the text.
        grid.feed("\x1b[2;1H\x1b[90m│\x1b[0m Hello World \x1b[90m│\x1b[0m".as_bytes());
        let tree = grid.to_render_tree(12, 20, 16, 16);
        let row2 = tree.iter().find(|n| n.id == "tui-row#1").expect("row 2");
        assert_eq!(
            row2.style.get("color").map(String::as_str),
            Some("rgb(244, 244, 245)")
        );
    }
}
