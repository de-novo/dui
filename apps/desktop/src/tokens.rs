//! Exact color palette tokens ported 1:1 from `apps/canvas/src/styles/tokens.css`.

use gpui_kit::{Hsla, Rgba};

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub background: Hsla,
    pub foreground: Hsla,
    pub card: Hsla,
    pub muted_foreground: Hsla,
    pub border: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub sidebar: Hsla,
    pub sidebar_foreground: Hsla,
    pub sidebar_muted: Hsla,
    pub sidebar_line: Hsla,
    pub glass: Hsla,
    pub glass_surface: Hsla,
    pub glass_line: Hsla,
    pub capture_surface: Hsla,
    pub capture_rim: Hsla,
    pub map_link: Hsla,
    pub map_grid: Hsla,
    pub scrim: Hsla,
}

fn rgb(hex: u32) -> Hsla {
    Rgba {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> Hsla {
    Rgba {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a,
    }
    .into()
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            background: rgb(0x171717),
            foreground: rgb(0xf1f1f1),
            card: rgb(0x202020),
            muted_foreground: rgb(0xbdbdbd),
            border: rgb(0x444444),
            primary: rgb(0xef803f),
            primary_foreground: rgb(0x21170f),
            sidebar: rgba(32, 32, 32, 0.90),
            sidebar_foreground: rgb(0xf5f5f5),
            sidebar_muted: rgb(0xc4c4c4),
            sidebar_line: rgba(255, 255, 255, 0.12),
            glass: rgba(26, 26, 26, 0.92),
            glass_surface: rgba(255, 255, 255, 0.06),
            glass_line: rgba(255, 255, 255, 0.13),
            capture_surface: rgba(49, 49, 49, 0.92),
            capture_rim: rgba(255, 255, 255, 0.12),
            map_link: rgb(0xaaaaaa),
            map_grid: rgb(0x404040),
            scrim: rgba(0, 0, 0, 0.55),
        }
    } else {
        Palette {
            background: rgb(0xf8f8f8),
            foreground: rgb(0x191919),
            card: rgb(0xffffff),
            muted_foreground: rgb(0x565656),
            border: rgb(0xd4d4d4),
            primary: rgb(0xef803f),
            primary_foreground: rgb(0x21170f),
            sidebar: rgba(253, 253, 253, 0.92),
            sidebar_foreground: rgb(0x202020),
            sidebar_muted: rgb(0x525252),
            sidebar_line: rgba(30, 30, 30, 0.12),
            glass: rgba(255, 255, 255, 0.92),
            glass_surface: rgba(255, 255, 255, 0.72),
            glass_line: rgba(25, 25, 25, 0.12),
            capture_surface: rgba(255, 255, 255, 0.92),
            capture_rim: rgba(25, 25, 25, 0.12),
            map_link: rgb(0x717171),
            map_grid: rgb(0xcecece),
            scrim: rgba(0, 0, 0, 0.35),
        }
    }
}
