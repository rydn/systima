//! Colour palettes. Everything visual flows through a `Theme` so the look
//! can be swapped at runtime (`t`) and degraded to 256 colours on
//! terminals without truecolor.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::BorderType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Linear blend `self → other` by `t ∈ [0,1]`.
    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb(f(self.0, other.0), f(self.1, other.1), f(self.2, other.2))
    }

    /// Nearest colour in the xterm-256 palette (6×6×6 cube + grey ramp).
    pub fn to_indexed(self) -> u8 {
        let Rgb(r, g, b) = self;
        // Cube candidate.
        let q = |v: u8| -> (u8, u8) {
            // 0,95,135,175,215,255
            let idx = if v < 48 {
                0
            } else if v < 115 {
                1
            } else {
                ((v as u16 - 35) / 40) as u8
            };
            let val = if idx == 0 { 0 } else { 55 + idx * 40 };
            (idx, val)
        };
        let (ri, rv) = q(r);
        let (gi, gv) = q(g);
        let (bi, bv) = q(b);
        let cube_idx = 16 + 36 * ri + 6 * gi + bi;
        let cube_dist = dist((r, g, b), (rv, gv, bv));
        // Grey candidate.
        let avg = (r as u16 + g as u16 + b as u16) / 3;
        let gi = if avg < 8 {
            0
        } else if avg > 238 {
            23
        } else {
            ((avg - 8) / 10) as u8
        };
        let gv = 8 + gi * 10;
        let grey_idx = 232 + gi;
        let grey_dist = dist((r, g, b), (gv, gv, gv));
        if grey_dist < cube_dist {
            grey_idx
        } else {
            cube_idx
        }
    }
}

fn dist(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let d = |x: u8, y: u8| (x as i32 - y as i32).pow(2) as u32;
    d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    Cyberpunk,
    Matrix,
}

impl Palette {
    pub const ALL: [Palette; 2] = [Palette::Cyberpunk, Palette::Matrix];

    pub fn from_name(name: &str) -> Option<Palette> {
        match name.to_ascii_lowercase().as_str() {
            "cyberpunk" | "cyber" | "neon" => Some(Palette::Cyberpunk),
            "matrix" | "green" => Some(Palette::Matrix),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Palette::Cyberpunk => "cyberpunk",
            Palette::Matrix => "matrix",
        }
    }

    pub fn next(self) -> Palette {
        let i = Palette::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Palette::ALL[(i + 1) % Palette::ALL.len()]
    }
}

/// A resolved palette. Colours are stored as RGB and converted on demand
/// (`color()`), so a single flag handles the 256-colour fallback.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub palette: Palette,
    pub truecolor: bool,
    pub bg: Rgb,
    pub panel: Rgb,
    pub border: Rgb,
    pub border_focus: Rgb,
    pub title: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub accent: Rgb,
    pub accent2: Rgb,
    pub warn: Rgb,
    pub danger: Rgb,
    pub good: Rgb,
    pub selection_bg: Rgb,
    /// Vertical gradient for graphs: bottom → mid → top.
    pub grad: [Rgb; 3],
    pub border_type: BorderType,
}

impl Theme {
    pub fn new(palette: Palette, truecolor: bool) -> Theme {
        match palette {
            Palette::Cyberpunk => Theme {
                palette,
                truecolor,
                bg: Rgb(0x0b, 0x01, 0x18),
                panel: Rgb(0x12, 0x06, 0x26),
                border: Rgb(0x5a, 0x2a, 0x8a),
                border_focus: Rgb(0xff, 0x2a, 0x6d),
                title: Rgb(0xff, 0x2a, 0x6d),
                text: Rgb(0xd1, 0xf7, 0xff),
                muted: Rgb(0x6b, 0x5b, 0x95),
                accent: Rgb(0x05, 0xd9, 0xe8),
                accent2: Rgb(0xff, 0x2a, 0x6d),
                warn: Rgb(0xf9, 0xf8, 0x71),
                danger: Rgb(0xff, 0x3b, 0x3b),
                good: Rgb(0x39, 0xff, 0x14),
                selection_bg: Rgb(0x3a, 0x0c, 0x5c),
                grad: [
                    Rgb(0x3b, 0x1a, 0x6e),
                    Rgb(0x05, 0xd9, 0xe8),
                    Rgb(0xff, 0x2a, 0x6d),
                ],
                border_type: BorderType::Rounded,
            },
            Palette::Matrix => Theme {
                palette,
                truecolor,
                bg: Rgb(0x02, 0x08, 0x02),
                panel: Rgb(0x04, 0x10, 0x04),
                border: Rgb(0x0f, 0x5a, 0x1f),
                border_focus: Rgb(0x39, 0xff, 0x14),
                title: Rgb(0x39, 0xff, 0x14),
                text: Rgb(0xc8, 0xff, 0xc8),
                muted: Rgb(0x2e, 0x7d, 0x32),
                accent: Rgb(0x39, 0xff, 0x14),
                accent2: Rgb(0x9c, 0xff, 0x57),
                warn: Rgb(0xff, 0xe6, 0x00),
                danger: Rgb(0xff, 0x3b, 0x3b),
                good: Rgb(0x39, 0xff, 0x14),
                selection_bg: Rgb(0x0b, 0x3d, 0x0b),
                grad: [
                    Rgb(0x0a, 0x3a, 0x0a),
                    Rgb(0x1f, 0xb3, 0x1f),
                    Rgb(0xb6, 0xff, 0x9c),
                ],
                border_type: BorderType::Double,
            },
        }
    }

    #[inline]
    pub fn color(&self, c: Rgb) -> Color {
        if self.truecolor {
            Color::Rgb(c.0, c.1, c.2)
        } else {
            Color::Indexed(c.to_indexed())
        }
    }

    /// Gradient colour at `t ∈ [0,1]` (0 = bottom of a graph).
    pub fn grad_at(&self, t: f32) -> Rgb {
        if t < 0.5 {
            self.grad[0].mix(self.grad[1], t * 2.0)
        } else {
            self.grad[1].mix(self.grad[2], (t - 0.5) * 2.0)
        }
    }

    /// Colour for a utilisation level `t ∈ [0,1]`: accent → accent2 → warn.
    pub fn by_load(&self, t: f32) -> Rgb {
        if t < 0.6 {
            self.accent.mix(self.accent2, t / 0.6)
        } else {
            self.accent2.mix(self.warn, (t - 0.6) / 0.4)
        }
    }

    // --- ready-made styles -------------------------------------------------

    pub fn base(&self) -> Style {
        Style::new()
            .fg(self.color(self.text))
            .bg(self.color(self.bg))
    }
    pub fn panel_style(&self) -> Style {
        Style::new()
            .fg(self.color(self.text))
            .bg(self.color(self.panel))
    }
    pub fn border_style(&self, focused: bool) -> Style {
        let c = if focused {
            self.border_focus
        } else {
            self.border
        };
        Style::new().fg(self.color(c))
    }
    pub fn title_style(&self) -> Style {
        Style::new()
            .fg(self.color(self.title))
            .add_modifier(Modifier::BOLD)
    }
    pub fn text_style(&self) -> Style {
        Style::new().fg(self.color(self.text))
    }
    pub fn muted_style(&self) -> Style {
        Style::new().fg(self.color(self.muted))
    }
    pub fn accent_style(&self) -> Style {
        Style::new().fg(self.color(self.accent))
    }
    pub fn accent2_style(&self) -> Style {
        Style::new().fg(self.color(self.accent2))
    }
    pub fn warn_style(&self) -> Style {
        Style::new()
            .fg(self.color(self.warn))
            .add_modifier(Modifier::BOLD)
    }
    pub fn danger_style(&self) -> Style {
        Style::new()
            .fg(self.color(self.danger))
            .add_modifier(Modifier::BOLD)
    }
    pub fn selection_style(&self) -> Style {
        Style::new()
            .fg(self.color(self.text))
            .bg(self.color(self.selection_bg))
            .add_modifier(Modifier::BOLD)
    }
    pub fn header_bar(&self) -> Style {
        Style::new()
            .fg(self.color(self.bg))
            .bg(self.color(self.accent))
            .add_modifier(Modifier::BOLD)
    }
}

/// Detect truecolor support from the environment.
pub fn env_truecolor() -> bool {
    match std::env::var("COLORTERM") {
        Ok(v) => {
            let v = v.to_ascii_lowercase();
            v.contains("truecolor") || v.contains("24bit")
        }
        Err(_) => {
            // Many modern terminals set TERM_PROGRAM; Terminal.app does not do truecolor.
            matches!(
                std::env::var("TERM_PROGRAM").as_deref(),
                Ok("iTerm.app")
                    | Ok("WezTerm")
                    | Ok("ghostty")
                    | Ok("vscode")
                    | Ok("Hyper")
                    | Ok("kitty")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_endpoints_and_midpoint() {
        let a = Rgb(0, 0, 0);
        let b = Rgb(200, 100, 50);
        assert_eq!(a.mix(b, 0.0), a);
        assert_eq!(a.mix(b, 1.0), b);
        assert_eq!(a.mix(b, 0.5), Rgb(100, 50, 25));
        assert_eq!(a.mix(b, 7.0), b);
    }

    #[test]
    fn indexed_fallback_hits_known_cells() {
        assert_eq!(Rgb(0, 0, 0).to_indexed(), 16);
        assert_eq!(Rgb(255, 255, 255).to_indexed(), 231);
        assert_eq!(Rgb(255, 0, 0).to_indexed(), 196);
        assert_eq!(Rgb(0, 0, 255).to_indexed(), 21);
        assert_eq!(Rgb(128, 128, 128).to_indexed(), 244);
    }

    #[test]
    fn palette_lookup_and_cycle() {
        assert_eq!(Palette::from_name("Matrix"), Some(Palette::Matrix));
        assert_eq!(Palette::from_name("nope"), None);
        assert_eq!(Palette::Cyberpunk.next(), Palette::Matrix);
        assert_eq!(Palette::Matrix.next(), Palette::Cyberpunk);
    }

    #[test]
    fn theme_color_mode() {
        let t = Theme::new(Palette::Cyberpunk, true);
        assert!(matches!(t.color(t.accent), Color::Rgb(..)));
        let t = Theme::new(Palette::Cyberpunk, false);
        assert!(matches!(t.color(t.accent), Color::Indexed(_)));
        assert_eq!(t.grad_at(0.0), t.grad[0]);
        assert_eq!(t.grad_at(1.0), t.grad[2]);
    }
}
