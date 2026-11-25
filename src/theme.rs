//! Look-and-feel themes: colours, font, wording style and the animated
//! background. The chosen theme is remembered between runs.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Stroke};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, PartialEq)]
pub enum Backdrop {
    /// falling green 0s and 1s
    Rain,
    /// flat colour
    Plain,
    /// twinkling stars
    Stars,
    /// smooth top-to-bottom colour blend
    Gradient(Color32, Color32),
    /// bubbles drifting up over a blend
    Bubbles(Color32, Color32),
    /// little hearts floating up over a blend
    Hearts(Color32, Color32),
    /// flakes or petals drifting down over a blend (top, bottom, particle colour)
    Falling(Color32, Color32, Color32),
    /// glowing fireflies wandering over a blend
    Fireflies(Color32, Color32),
    /// retro sunset with a moving neon grid (sky top, horizon, grid lines)
    Grid(Color32, Color32, Color32),
    /// glowing sparks rising from the bottom
    Embers(Color32, Color32),
    /// slowly waving northern-lights ribbons over a night sky
    Aurora(Color32, Color32),
}

pub struct Theme {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub accent: Color32,
    pub accent_dark: Color32,
    pub bg: Color32,
    pub card: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
    /// text drawn on top of the accent colour
    pub ink: Color32,
    pub dark: bool,
    /// monospace font everywhere
    pub mono: bool,
    /// terminal-style wording ("> APPLE_ID", "[ EXECUTE ]")
    pub terminal: bool,
    pub radius: u8,
    pub backdrop: Backdrop,
    // widget fills
    pub w_window: Color32,
    pub w_extreme: Color32,
    pub w_inactive: Color32,
    pub w_hovered: Color32,
    pub w_active: Color32,
    pub w_selection: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub static THEMES: [Theme; 23] = [
    Theme {
        key: "hacker",
        name: "Hacker",
        icon: "💻",
        accent: rgb(0, 255, 70),
        accent_dark: rgb(0, 170, 50),
        bg: rgb(2, 8, 3),
        card: Color32::from_rgba_premultiplied(3, 14, 6, 225),
        border: rgb(0, 120, 40),
        text: rgb(120, 255, 140),
        muted: rgb(40, 160, 70),
        good: rgb(0, 255, 70),
        warn: rgb(255, 200, 0),
        bad: rgb(255, 60, 60),
        ink: rgb(0, 12, 2),
        dark: true,
        mono: true,
        terminal: true,
        radius: 2,
        backdrop: Backdrop::Rain,
        w_window: rgb(2, 14, 5),
        w_extreme: rgb(0, 6, 1),
        w_inactive: rgb(0, 22, 8),
        w_hovered: rgb(0, 45, 15),
        w_active: rgb(0, 70, 22),
        w_selection: rgb(0, 90, 30),
    },
    Theme {
        key: "normal",
        name: "Normal",
        icon: "☀",
        accent: rgb(0, 122, 255),
        accent_dark: rgb(0, 96, 210),
        bg: rgb(242, 243, 247),
        card: Color32::WHITE,
        border: rgb(214, 218, 226),
        text: rgb(30, 33, 40),
        muted: rgb(110, 116, 128),
        good: rgb(30, 150, 70),
        warn: rgb(200, 120, 0),
        bad: rgb(210, 50, 50),
        ink: Color32::WHITE,
        dark: false,
        mono: false,
        terminal: false,
        radius: 6,
        backdrop: Backdrop::Plain,
        w_window: Color32::WHITE,
        w_extreme: Color32::WHITE,
        w_inactive: rgb(250, 250, 252),
        w_hovered: rgb(238, 242, 250),
        w_active: rgb(225, 233, 248),
        w_selection: rgb(190, 220, 255),
    },
    Theme {
        key: "blackwhite",
        name: "Black & White",
        icon: "⬛",
        accent: rgb(0, 0, 0),
        accent_dark: rgb(0, 0, 0),
        bg: rgb(255, 255, 255),
        card: Color32::WHITE,
        border: rgb(0, 0, 0),
        text: rgb(0, 0, 0),
        muted: rgb(90, 90, 90),
        good: rgb(0, 0, 0),
        warn: rgb(80, 80, 80),
        bad: rgb(0, 0, 0),
        ink: Color32::WHITE,
        dark: false,
        mono: false,
        terminal: false,
        radius: 0,
        backdrop: Backdrop::Plain,
        w_window: Color32::WHITE,
        w_extreme: Color32::WHITE,
        w_inactive: rgb(244, 244, 244),
        w_hovered: rgb(232, 232, 232),
        w_active: rgb(210, 210, 210),
        w_selection: rgb(200, 200, 200),
    },
    Theme {
        key: "whiteonblack",
        name: "White on Black",
        icon: "⬜",
        accent: rgb(255, 255, 255),
        accent_dark: rgb(255, 255, 255),
        bg: rgb(0, 0, 0),
        card: rgb(0, 0, 0),
        border: rgb(255, 255, 255),
        text: rgb(255, 255, 255),
        muted: rgb(165, 165, 165),
        good: rgb(255, 255, 255),
        warn: rgb(175, 175, 175),
        bad: rgb(255, 255, 255),
        ink: rgb(0, 0, 0),
        dark: true,
        mono: false,
        terminal: false,
        radius: 0,
        backdrop: Backdrop::Plain,
        w_window: rgb(0, 0, 0),
        w_extreme: rgb(0, 0, 0),
        w_inactive: rgb(14, 14, 14),
        w_hovered: rgb(40, 40, 40),
        w_active: rgb(70, 70, 70),
        w_selection: rgb(85, 85, 85),
    },
    Theme {
        key: "pink",
        name: "Pink",
        icon: "💗",
        accent: rgb(236, 64, 140),
        accent_dark: rgb(200, 40, 115),
        bg: rgb(255, 228, 240),
        card: Color32::from_rgba_premultiplied(242, 242, 242, 242),
        border: rgb(246, 170, 205),
        text: rgb(90, 20, 55),
        muted: rgb(170, 90, 130),
        good: rgb(30, 140, 90),
        warn: rgb(200, 110, 0),
        bad: rgb(200, 30, 60),
        ink: Color32::WHITE,
        dark: false,
        mono: false,
        terminal: false,
        radius: 14,
        backdrop: Backdrop::Hearts(rgb(255, 214, 232), rgb(250, 160, 200)),
        w_window: rgb(255, 245, 250),
        w_extreme: Color32::WHITE,
        w_inactive: rgb(255, 240, 247),
        w_hovered: rgb(255, 222, 238),
        w_active: rgb(252, 200, 225),
        w_selection: rgb(250, 190, 220),
    },
    Theme {
        key: "midnight",
        name: "Midnight",
        icon: "🌙",
        accent: rgb(140, 110, 255),
        accent_dark: rgb(110, 80, 230),
        bg: rgb(10, 12, 28),
        card: Color32::from_rgba_premultiplied(20, 24, 48, 235),
        border: rgb(58, 64, 115),
        text: rgb(226, 229, 246),
        muted: rgb(140, 148, 190),
        good: rgb(80, 220, 150),
        warn: rgb(255, 190, 80),
        bad: rgb(255, 95, 110),
        ink: Color32::WHITE,
        dark: true,
        mono: false,
        terminal: false,
        radius: 8,
        backdrop: Backdrop::Stars,
        w_window: rgb(20, 24, 48),
        w_extreme: rgb(10, 12, 26),
        w_inactive: rgb(28, 32, 62),
        w_hovered: rgb(40, 46, 88),
        w_active: rgb(55, 60, 115),
        w_selection: rgb(70, 55, 150),
    },
    Theme {
        key: "sunset",
        name: "Sunset",
        icon: "🌅",
        accent: rgb(240, 90, 60),
        accent_dark: rgb(205, 65, 45),
        bg: rgb(250, 160, 130),
        card: Color32::from_rgba_premultiplied(240, 240, 240, 240),
        border: rgb(240, 190, 170),
        text: rgb(60, 30, 40),
        muted: rgb(140, 90, 100),
        good: rgb(30, 140, 70),
        warn: rgb(200, 110, 0),
        bad: rgb(200, 40, 60),
        ink: Color32::WHITE,
        dark: false,
        mono: false,
        terminal: false,
        radius: 12,
        backdrop: Backdrop::Gradient(rgb(255, 196, 120), rgb(232, 92, 140)),
        w_window: rgb(255, 250, 247),
        w_extreme: Color32::WHITE,
        w_inactive: rgb(255, 246, 241),
        w_hovered: rgb(255, 234, 224),
        w_active: rgb(255, 220, 206),
        w_selection: rgb(255, 200, 180),
    },
    Theme {
        key: "ocean",
        name: "Ocean",
        icon: "🌊",
        accent: rgb(0, 200, 220),
        accent_dark: rgb(0, 155, 175),
        bg: rgb(3, 30, 48),
        card: Color32::from_rgba_premultiplied(5, 40, 58, 230),
        border: rgb(30, 100, 125),
        text: rgb(220, 245, 250),
        muted: rgb(120, 170, 190),
        good: rgb(60, 220, 160),
        warn: rgb(255, 200, 90),
        bad: rgb(255, 100, 100),
        ink: rgb(2, 25, 35),
        dark: true,
        mono: false,
        terminal: false,
        radius: 10,
        backdrop: Backdrop::Bubbles(rgb(4, 58, 86), rgb(2, 16, 30)),
        w_window: rgb(6, 40, 58),
        w_extreme: rgb(3, 25, 38),
        w_inactive: rgb(8, 55, 78),
        w_hovered: rgb(12, 75, 100),
        w_active: rgb(16, 95, 125),
        w_selection: rgb(0, 110, 130),
    },
    // ---- popular open-source colour schemes (palettes recreated by hand) ----
    Theme {
        key: "catppuccin-mocha",
        name: "Catppuccin Mocha",
        icon: "☕",
        accent: rgb(203, 166, 247),
        accent_dark: rgb(180, 140, 230),
        bg: rgb(30, 30, 46),
        card: Color32::from_rgba_premultiplied(45, 46, 63, 235),
        border: rgb(69, 71, 90),
        text: rgb(205, 214, 244),
        muted: rgb(166, 173, 200),
        good: rgb(166, 227, 161),
        warn: rgb(249, 226, 175),
        bad: rgb(243, 139, 168),
        ink: rgb(30, 30, 46),
        dark: true,
        mono: false,
        terminal: false,
        radius: 10,
        backdrop: Backdrop::Gradient(rgb(40, 38, 62), rgb(17, 17, 27)),
        w_window: rgb(49, 50, 68),
        w_extreme: rgb(24, 24, 37),
        w_inactive: rgb(49, 50, 68),
        w_hovered: rgb(69, 71, 90),
        w_active: rgb(88, 91, 112),
        w_selection: rgb(88, 70, 130),
    },
    Theme {
        key: "catppuccin-latte",
        name: "Catppuccin Latte",
        icon: "🍰",
        accent: rgb(136, 57, 239),
        accent_dark: rgb(114, 40, 210),
        bg: rgb(239, 241, 245),
        card: rgb(250, 251, 253),
        border: rgb(204, 208, 218),
        text: rgb(76, 79, 105),
        muted: rgb(140, 143, 161),
        good: rgb(64, 160, 43),
        warn: rgb(223, 142, 29),
        bad: rgb(210, 15, 57),
        ink: Color32::WHITE,
        dark: false,
        mono: false,
        terminal: false,
        radius: 10,
        backdrop: Backdrop::Gradient(rgb(239, 241, 245), rgb(220, 224, 232)),
        w_window: Color32::WHITE,
        w_extreme: Color32::WHITE,
        w_inactive: rgb(230, 233, 239),
        w_hovered: rgb(220, 224, 232),
        w_active: rgb(204, 208, 218),
        w_selection: rgb(210, 190, 245),
    },
    Theme {
        key: "nord",
        name: "Nord",
        icon: "❄",
        accent: rgb(136, 192, 208),
        accent_dark: rgb(94, 129, 172),
        bg: rgb(46, 52, 64),
        card: Color32::from_rgba_premultiplied(54, 61, 75, 235),
        border: rgb(76, 86, 106),
        text: rgb(236, 239, 244),
        muted: rgb(160, 172, 192),
        good: rgb(163, 190, 140),
        warn: rgb(235, 203, 139),
        bad: rgb(191, 97, 106),
        ink: rgb(46, 52, 64),
        dark: true,
        mono: false,
        terminal: false,
        radius: 6,
        backdrop: Backdrop::Falling(rgb(59, 66, 82), rgb(36, 41, 51), rgb(236, 239, 244)),
        w_window: rgb(59, 66, 82),
        w_extreme: rgb(40, 45, 56),
        w_inactive: rgb(59, 66, 82),
        w_hovered: rgb(67, 76, 94),
        w_active: rgb(76, 86, 106),
        w_selection: rgb(94, 129, 172),
    },
    Theme {
        key: "everforest",
        name: "Everforest",
        icon: "🌲",
        accent: rgb(167, 192, 128),
        accent_dark: rgb(131, 158, 96),
        bg: rgb(45, 53, 59),
        card: Color32::from_rgba_premultiplied(48, 58, 63, 235),
        border: rgb(71, 82, 88),
        text: rgb(211, 198, 170),
        muted: rgb(133, 146, 137),
        good: rgb(167, 192, 128),
        warn: rgb(219, 188, 127),
        bad: rgb(230, 126, 128),
        ink: rgb(35, 42, 46),
        dark: true,
        mono: false,
        terminal: false,
        radius: 8,
        backdrop: Backdrop::Fireflies(rgb(45, 53, 59), rgb(28, 34, 38)),
        w_window: rgb(52, 63, 68),
        w_extreme: rgb(39, 46, 51),
        w_inactive: rgb(52, 63, 68),
        w_hovered: rgb(61, 72, 77),
        w_active: rgb(71, 82, 88),
        w_selection: rgb(80, 100, 70),
    },
    Theme {
        key: "solarized-dark",
        name: "Solarized Dark",
        icon: "🌗",
        accent: rgb(38, 139, 210),
        accent_dark: rgb(30, 110, 170),
        bg: rgb(0, 43, 54),
        card: Color32::from_rgba_premultiplied(7, 51, 62, 240),
        border: rgb(88, 110, 117),
        text: rgb(170, 183, 183),
        muted: rgb(101, 123, 131),
        good: rgb(133, 153, 0),
        warn: rgb(181, 137, 0),
        bad: rgb(220, 50, 47),
        ink: rgb(253, 246, 227),
        dark: true,
        mono: false,
        terminal: false,
        radius: 4,
        backdrop: Backdrop::Gradient(rgb(0, 50, 62), rgb(0, 32, 42)),
        w_window: rgb(7, 54, 66),
        w_extreme: rgb(0, 35, 45),
        w_inactive: rgb(7, 54, 66),
        w_hovered: rgb(12, 66, 80),
        w_active: rgb(20, 80, 96),
        w_selection: rgb(38, 90, 130),
    },
    Theme {
        key: "solarized-light",
        name: "Solarized Light",
        icon: "⛅",
        accent: rgb(38, 139, 210),
        accent_dark: rgb(30, 110, 170),
        bg: rgb(253, 246, 227),
        card: rgb(238, 232, 213),
        border: rgb(205, 198, 176),
        text: rgb(60, 82, 90),
        muted: rgb(147, 161, 161),
        good: rgb(133, 153, 0),
        warn: rgb(181, 137, 0),
        bad: rgb(220, 50, 47),
        ink: Color32::WHITE,
        dark: false,
        mono: false,
        terminal: false,
        radius: 4,
        backdrop: Backdrop::Plain,
        w_window: rgb(253, 246, 227),
        w_extreme: rgb(253, 250, 240),
        w_inactive: rgb(245, 238, 218),
        w_hovered: rgb(238, 232, 213),
        w_active: rgb(225, 218, 195),
        w_selection: rgb(200, 220, 235),
    },
    Theme {
        key: "dracula",
        name: "Dracula",
        icon: "👻",
        accent: rgb(189, 147, 249),
        accent_dark: rgb(160, 120, 230),
        bg: rgb(40, 42, 54),
        card: Color32::from_rgba_premultiplied(61, 64, 81, 230),
        border: rgb(98, 114, 164),
        text: rgb(248, 248, 242),
        muted: rgb(150, 160, 200),
        good: rgb(80, 250, 123),
        warn: rgb(241, 250, 140),
        bad: rgb(255, 85, 85),
        ink: rgb(40, 42, 54),
        dark: true,
        mono: false,
        terminal: false,
        radius: 8,
        backdrop: Backdrop::Gradient(rgb(52, 50, 72), rgb(28, 29, 38)),
        w_window: rgb(68, 71, 90),
        w_extreme: rgb(33, 34, 44),
        w_inactive: rgb(68, 71, 90),
        w_hovered: rgb(80, 84, 106),
        w_active: rgb(98, 114, 164),
        w_selection: rgb(98, 80, 150),
    },
    Theme {
        key: "tokyo-night",
        name: "Tokyo Night",
        icon: "🗼",
        accent: rgb(122, 162, 247),
        accent_dark: rgb(90, 130, 220),
        bg: rgb(26, 27, 38),
        card: Color32::from_rgba_premultiplied(33, 37, 54, 235),
        border: rgb(65, 72, 104),
        text: rgb(192, 202, 245),
        muted: rgb(120, 130, 175),
        good: rgb(158, 206, 106),
        warn: rgb(224, 175, 104),
        bad: rgb(247, 118, 142),
        ink: rgb(26, 27, 38),
        dark: true,
        mono: false,
        terminal: false,
        radius: 8,
        backdrop: Backdrop::Gradient(rgb(36, 40, 59), rgb(18, 18, 26)),
        w_window: rgb(36, 40, 59),
        w_extreme: rgb(22, 22, 30),
        w_inactive: rgb(36, 40, 59),
        w_hovered: rgb(47, 53, 78),
        w_active: rgb(65, 72, 104),
        w_selection: rgb(55, 75, 140),
    },
    Theme {
        key: "gruvbox",
        name: "Gruvbox",
        icon: "🍂",
        accent: rgb(254, 128, 25),
        accent_dark: rgb(214, 93, 14),
        bg: rgb(40, 40, 40),
        card: Color32::from_rgba_premultiplied(56, 53, 51, 240),
        border: rgb(102, 92, 84),
        text: rgb(235, 219, 178),
        muted: rgb(168, 153, 132),
        good: rgb(184, 187, 38),
        warn: rgb(250, 189, 47),
        bad: rgb(251, 73, 52),
        ink: rgb(40, 40, 40),
        dark: true,
        mono: false,
        terminal: false,
        radius: 4,
        backdrop: Backdrop::Gradient(rgb(50, 48, 47), rgb(29, 32, 33)),
        w_window: rgb(60, 56, 54),
        w_extreme: rgb(29, 32, 33),
        w_inactive: rgb(60, 56, 54),
        w_hovered: rgb(80, 73, 69),
        w_active: rgb(102, 92, 84),
        w_selection: rgb(120, 80, 40),
    },
    Theme {
        key: "rose-pine",
        name: "Rosé Pine",
        icon: "🌹",
        accent: rgb(235, 188, 186),
        accent_dark: rgb(214, 160, 160),
        bg: rgb(25, 23, 36),
        card: Color32::from_rgba_premultiplied(29, 27, 43, 240),
        border: rgb(82, 79, 103),
        text: rgb(224, 222, 244),
        muted: rgb(144, 140, 170),
        good: rgb(156, 207, 216),
        warn: rgb(246, 193, 119),
        bad: rgb(235, 111, 146),
        ink: rgb(25, 23, 36),
        dark: true,
        mono: false,
        terminal: false,
        radius: 12,
        backdrop: Backdrop::Falling(rgb(31, 29, 46), rgb(20, 18, 30), rgb(235, 188, 186)),
        w_window: rgb(31, 29, 46),
        w_extreme: rgb(21, 19, 32),
        w_inactive: rgb(38, 35, 58),
        w_hovered: rgb(49, 46, 72),
        w_active: rgb(64, 61, 82),
        w_selection: rgb(110, 80, 120),
    },
    Theme {
        key: "one-dark",
        name: "One Dark",
        icon: "⚛",
        accent: rgb(97, 175, 239),
        accent_dark: rgb(70, 145, 210),
        bg: rgb(40, 44, 52),
        card: Color32::from_rgba_premultiplied(32, 36, 42, 245),
        border: rgb(62, 68, 81),
        text: rgb(171, 178, 191),
        muted: rgb(120, 127, 140),
        good: rgb(152, 195, 121),
        warn: rgb(229, 192, 123),
        bad: rgb(224, 108, 117),
        ink: rgb(33, 37, 43),
        dark: true,
        mono: false,
        terminal: false,
        radius: 6,
        backdrop: Backdrop::Plain,
        w_window: rgb(33, 37, 43),
        w_extreme: rgb(27, 30, 35),
        w_inactive: rgb(44, 49, 58),
        w_hovered: rgb(55, 61, 72),
        w_active: rgb(62, 68, 81),
        w_selection: rgb(60, 90, 130),
    },
    Theme {
        key: "monokai",
        name: "Monokai",
        icon: "🎨",
        accent: rgb(166, 226, 46),
        accent_dark: rgb(130, 190, 30),
        bg: rgb(39, 40, 34),
        card: Color32::from_rgba_premultiplied(57, 56, 46, 235),
        border: rgb(117, 113, 94),
        text: rgb(248, 248, 242),
        muted: rgb(150, 146, 120),
        good: rgb(166, 226, 46),
        warn: rgb(230, 219, 116),
        bad: rgb(249, 38, 114),
        ink: rgb(39, 40, 34),
        dark: true,
        mono: false,
        terminal: false,
        radius: 4,
        backdrop: Backdrop::Gradient(rgb(45, 46, 39), rgb(30, 31, 26)),
        w_window: rgb(62, 61, 50),
        w_extreme: rgb(30, 31, 26),
        w_inactive: rgb(62, 61, 50),
        w_hovered: rgb(76, 75, 62),
        w_active: rgb(100, 98, 80),
        w_selection: rgb(73, 72, 62),
    },
    // ---- ipabazooka originals ----
    Theme {
        key: "synthwave",
        name: "Synthwave",
        icon: "🕹",
        accent: rgb(255, 126, 219),
        accent_dark: rgb(220, 90, 190),
        bg: rgb(36, 27, 47),
        card: Color32::from_rgba_premultiplied(39, 29, 51, 225),
        border: rgb(114, 63, 150),
        text: rgb(244, 238, 255),
        muted: rgb(180, 160, 210),
        good: rgb(114, 241, 184),
        warn: rgb(254, 222, 93),
        bad: rgb(254, 68, 80),
        ink: rgb(36, 27, 47),
        dark: true,
        mono: false,
        terminal: false,
        radius: 6,
        backdrop: Backdrop::Grid(rgb(26, 12, 48), rgb(110, 30, 100), rgb(255, 60, 200)),
        w_window: rgb(44, 33, 58),
        w_extreme: rgb(28, 20, 38),
        w_inactive: rgb(52, 38, 72),
        w_hovered: rgb(70, 48, 96),
        w_active: rgb(95, 60, 125),
        w_selection: rgb(120, 50, 130),
    },
    Theme {
        key: "lava",
        name: "Lava",
        icon: "🌋",
        accent: rgb(255, 96, 32),
        accent_dark: rgb(220, 64, 16),
        bg: rgb(20, 6, 4),
        card: Color32::from_rgba_premultiplied(34, 13, 9, 230),
        border: rgb(110, 40, 20),
        text: rgb(255, 224, 205),
        muted: rgb(200, 140, 110),
        good: rgb(140, 220, 110),
        warn: rgb(255, 190, 60),
        bad: rgb(255, 70, 70),
        ink: rgb(20, 6, 4),
        dark: true,
        mono: false,
        terminal: false,
        radius: 8,
        backdrop: Backdrop::Embers(rgb(30, 6, 4), rgb(12, 3, 2)),
        w_window: rgb(38, 14, 10),
        w_extreme: rgb(16, 5, 3),
        w_inactive: rgb(48, 18, 12),
        w_hovered: rgb(70, 26, 16),
        w_active: rgb(100, 36, 20),
        w_selection: rgb(130, 45, 20),
    },
    Theme {
        key: "aurora",
        name: "Aurora",
        icon: "🌌",
        accent: rgb(111, 255, 210),
        accent_dark: rgb(60, 210, 170),
        bg: rgb(6, 16, 30),
        card: Color32::from_rgba_premultiplied(9, 23, 39, 225),
        border: rgb(40, 90, 110),
        text: rgb(220, 245, 250),
        muted: rgb(130, 170, 190),
        good: rgb(111, 255, 170),
        warn: rgb(255, 214, 110),
        bad: rgb(255, 110, 140),
        ink: rgb(6, 16, 30),
        dark: true,
        mono: false,
        terminal: false,
        radius: 10,
        backdrop: Backdrop::Aurora(rgb(4, 10, 24), rgb(8, 26, 40)),
        w_window: rgb(12, 28, 46),
        w_extreme: rgb(5, 13, 24),
        w_inactive: rgb(14, 34, 54),
        w_hovered: rgb(20, 48, 72),
        w_active: rgb(28, 64, 92),
        w_selection: rgb(30, 90, 100),
    },
];

static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// The theme in use right now.
pub fn th() -> &'static Theme {
    &THEMES[CURRENT.load(Ordering::Relaxed).min(THEMES.len() - 1)]
}

pub fn current_index() -> usize {
    CURRENT.load(Ordering::Relaxed)
}

/// Picks wording: terminal style for the hacker theme, plain otherwise.
pub fn lbl(terminal: &'static str, plain: &'static str) -> &'static str {
    if th().terminal { terminal } else { plain }
}

/// Font for painted text, following the theme.
pub fn font(size: f32) -> FontId {
    if th().mono { FontId::monospace(size) } else { FontId::proportional(size) }
}

fn saved_path() -> std::path::PathBuf {
    crate::paths::data_dir().join("theme.txt")
}

/// Loads the remembered theme (Hacker if none was saved).
pub fn load_saved() {
    if let Ok(s) = std::fs::read_to_string(saved_path()) {
        if let Some(i) = THEMES.iter().position(|t| t.key == s.trim()) {
            CURRENT.store(i, Ordering::Relaxed);
        }
    }
}

/// Switches theme, applies it and remembers it.
pub fn select(ctx: &egui::Context, index: usize) {
    if index >= THEMES.len() {
        return;
    }
    CURRENT.store(index, Ordering::Relaxed);
    apply(ctx);
    let p = saved_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, THEMES[index].key);
}

/// Pushes the current theme into egui's style (colours, corners, fonts).
pub fn apply(ctx: &egui::Context) {
    let t = th();
    let mut v = if t.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.override_text_color = Some(t.text);
    v.panel_fill = t.bg;
    v.window_fill = t.w_window;
    v.window_stroke = Stroke::new(1.0_f32, if t.dark { t.accent } else { t.border });
    v.window_corner_radius = CornerRadius::same(t.radius.saturating_add(2));
    v.extreme_bg_color = t.w_extreme;
    v.faint_bg_color = t.w_inactive;
    v.hyperlink_color = t.accent;
    for (w, fill, stroke) in [
        (&mut v.widgets.noninteractive, t.w_window, t.border),
        (&mut v.widgets.inactive, t.w_inactive, t.border),
        (&mut v.widgets.hovered, t.w_hovered, t.accent),
        (&mut v.widgets.active, t.w_active, t.accent),
        (&mut v.widgets.open, t.w_hovered, t.accent),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0_f32, stroke);
        w.fg_stroke = Stroke::new(1.0_f32, t.text);
        w.corner_radius = CornerRadius::same(t.radius);
    }
    v.selection.bg_fill = t.w_selection;
    v.selection.stroke = Stroke::new(1.0_f32, t.accent);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::Vec2::new(8.0, 6.0);
        s.spacing.button_padding = egui::Vec2::new(10.0, 4.0);
        s.spacing.interact_size.y = 24.0;
        for (style, font) in s.text_styles.iter_mut() {
            font.family = if t.mono || *style == egui::TextStyle::Monospace {
                egui::FontFamily::Monospace
            } else {
                egui::FontFamily::Proportional
            };
        }
    });
}

/// True when the background moves (so the window keeps redrawing).
pub fn animated() -> bool {
    !matches!(th().backdrop, Backdrop::Plain | Backdrop::Gradient(..))
}

fn hash(a: i64, b: i64) -> u64 {
    let mut x = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 32)
}

fn gradient(p: &egui::Painter, rect: egui::Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    p.add(egui::Shape::mesh(mesh));
}

/// Paints the theme's background behind everything.
pub fn paint_backdrop(ctx: &egui::Context, now: f64) {
    let screen = ctx.screen_rect();
    let p = ctx.layer_painter(egui::LayerId::background());
    let t = th();
    match t.backdrop {
        Backdrop::Plain => {
            p.rect_filled(screen, 0.0, t.bg);
        }
        Backdrop::Gradient(top, bottom) => {
            gradient(&p, screen, top, bottom);
        }
        Backdrop::Rain => {
            p.rect_filled(screen, 0.0, t.bg);
            paint_rain(&p, screen, now);
        }
        Backdrop::Stars => {
            gradient(&p, screen, Color32::from_rgb(24, 20, 58), t.bg);
            for i in 0..140i64 {
                let h = hash(i, 11);
                let x = screen.left() + (h % 10_000) as f32 / 10_000.0 * screen.width();
                let y = screen.top() + ((h >> 16) % 10_000) as f32 / 10_000.0 * screen.height();
                let speed = 0.6 + ((h >> 32) % 100) as f64 / 60.0;
                let phase = ((h >> 40) % 628) as f64 / 100.0;
                let tw = (0.55 + 0.45 * (now * speed + phase).sin()) as f32;
                let size = 0.6 + ((h >> 48) % 3) as f32 * 0.5;
                let a = (tw * 200.0) as u8;
                p.circle_filled(egui::pos2(x, y), size, Color32::from_rgba_unmultiplied(230, 225, 255, a));
            }
        }
        Backdrop::Bubbles(top, bottom) => {
            gradient(&p, screen, top, bottom);
            for i in 0..45i64 {
                let h = hash(i, 23);
                let r = 3.0 + (h % 12) as f32;
                let speed = 12.0 + ((h >> 8) % 30) as f64;
                let span = screen.height() as f64 + 2.0 * r as f64;
                let y0 = ((h >> 16) % 10_000) as f64 / 10_000.0 * span;
                let y = screen.bottom() as f64 + r as f64 - (y0 + now * speed) % span;
                let wobble = ((now * 0.8 + i as f64).sin() * 6.0) as f32;
                let x = screen.left() + ((h >> 32) % 10_000) as f32 / 10_000.0 * screen.width() + wobble;
                let c = egui::pos2(x, y as f32);
                p.circle_stroke(c, r, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(160, 230, 255, 70)));
                p.circle_filled(c - egui::vec2(r * 0.35, r * 0.35), r * 0.22, Color32::from_rgba_unmultiplied(220, 250, 255, 70));
            }
        }
        Backdrop::Hearts(top, bottom) => {
            gradient(&p, screen, top, bottom);
            for i in 0..34i64 {
                let h = hash(i, 41);
                let size = 5.0 + (h % 10) as f32; // half-width in points
                let speed = 10.0 + ((h >> 8) % 22) as f64;
                let span = screen.height() as f64 + 4.0 * size as f64;
                let y0 = ((h >> 16) % 10_000) as f64 / 10_000.0 * span;
                let y = screen.bottom() as f64 + 2.0 * size as f64 - (y0 + now * speed) % span;
                let sway = ((now * 0.7 + i as f64 * 1.3).sin() * 10.0) as f32;
                let x = screen.left() + ((h >> 32) % 10_000) as f32 / 10_000.0 * screen.width() + sway;
                let alpha = 60 + ((h >> 40) % 70) as u8;
                let color = if (h >> 50) % 3 == 0 {
                    Color32::from_rgba_unmultiplied(255, 255, 255, alpha)
                } else {
                    Color32::from_rgba_unmultiplied(236, 64, 140, alpha)
                };
                heart(&p, egui::pos2(x, y as f32), size, color);
            }
        }
        Backdrop::Falling(top, bottom, color) => {
            gradient(&p, screen, top, bottom);
            for i in 0..80i64 {
                let h = hash(i, 57);
                let r = 1.2 + (h % 30) as f32 / 10.0;
                let speed = 14.0 + ((h >> 8) % 34) as f64;
                let span = screen.height() as f64 + 20.0;
                let y0 = ((h >> 16) % 10_000) as f64 / 10_000.0 * span;
                let y = screen.top() as f64 - 10.0 + (y0 + now * speed) % span;
                let sway = ((now * 0.9 + i as f64 * 0.7).sin() * (6.0 + ((h >> 40) % 10) as f64)) as f32;
                let x = screen.left() + ((h >> 32) % 10_000) as f32 / 10_000.0 * screen.width() + sway;
                let a = 110 + ((h >> 44) % 120) as u8;
                p.circle_filled(egui::pos2(x, y as f32), r, with_alpha(color, a));
            }
        }
        Backdrop::Fireflies(top, bottom) => {
            gradient(&p, screen, top, bottom);
            for i in 0..32i64 {
                let h = hash(i, 71);
                let bx = (h % 10_000) as f32 / 10_000.0;
                let by = ((h >> 16) % 10_000) as f32 / 10_000.0;
                let f = 0.05 + ((h >> 32) % 100) as f64 / 1000.0;
                let x = screen.left() + screen.width() * (bx + 0.06 * (now * f * 6.0 + i as f64).sin() as f32);
                let y = screen.top() + screen.height() * (by + 0.05 * (now * f * 5.0 + i as f64 * 1.7).cos() as f32);
                let pulse = (0.5 + 0.5 * (now * (1.2 + ((h >> 40) % 10) as f64 * 0.15) + i as f64).sin()) as f32;
                let c = egui::pos2(x, y);
                p.circle_filled(c, 7.0, Color32::from_rgba_unmultiplied(220, 255, 120, (pulse * 35.0) as u8));
                p.circle_filled(c, 2.2, Color32::from_rgba_unmultiplied(240, 255, 170, (60.0 + pulse * 170.0) as u8));
            }
        }
        Backdrop::Grid(sky, horizon, line) => {
            let hy = screen.top() + screen.height() * 0.58;
            gradient(&p, egui::Rect::from_min_max(screen.left_top(), egui::pos2(screen.right(), hy)), sky, horizon);
            gradient(
                &p,
                egui::Rect::from_min_max(egui::pos2(screen.left(), hy), screen.right_bottom()),
                Color32::from_rgb(22, 8, 36),
                Color32::from_rgb(8, 2, 16),
            );
            // striped retro sun, cut off at the horizon
            let cx = screen.center().x;
            let r = screen.width().min(screen.height()) * 0.17;
            let sun_y = hy - r * 0.35;
            let slices = 44;
            for s in 0..slices {
                let y0 = sun_y - r + 2.0 * r * s as f32 / slices as f32;
                let y1 = (y0 + 2.0 * r / slices as f32).min(hy);
                if y0 >= hy {
                    break;
                }
                let rel = (y0 - sun_y) / r; // -1 (top) .. 1 (bottom)
                if rel > 0.05 && ((rel * 9.0) as i32) % 2 == 1 {
                    continue; // the gaps in the sun's lower half
                }
                let dy = (y0 + y1) / 2.0 - sun_y;
                let half = (r * r - dy * dy).max(0.0).sqrt();
                let t = s as f32 / slices as f32;
                let col = mix(Color32::from_rgb(255, 216, 92), Color32::from_rgb(255, 60, 170), t);
                p.rect_filled(egui::Rect::from_min_max(egui::pos2(cx - half, y0), egui::pos2(cx + half, y1)), 0.0, col);
            }
            // floor grid moving toward the viewer
            let n = 14;
            for k in 0..n {
                let t = ((k as f64 + now * 0.8) % n as f64) / n as f64;
                let y = hy + (screen.bottom() - hy) * (t * t) as f32;
                p.line_segment(
                    [egui::pos2(screen.left(), y), egui::pos2(screen.right(), y)],
                    Stroke::new(1.0 + t as f32, with_alpha(line, (60.0 + 150.0 * t) as u8)),
                );
            }
            for j in -16..=16 {
                let xb = cx + j as f32 * screen.width() * 0.09;
                let xt = cx + j as f32 * screen.width() * 0.012;
                p.line_segment([egui::pos2(xt, hy), egui::pos2(xb, screen.bottom())], Stroke::new(1.2_f32, with_alpha(line, 140)));
            }
            p.line_segment([egui::pos2(screen.left(), hy), egui::pos2(screen.right(), hy)], Stroke::new(2.0_f32, with_alpha(line, 220)));
        }
        Backdrop::Embers(top, bottom) => {
            gradient(&p, screen, top, bottom);
            // warm glow along the bottom edge
            let glow = egui::Rect::from_min_max(
                egui::pos2(screen.left(), screen.bottom() - screen.height() * 0.35),
                screen.right_bottom(),
            );
            gradient(&p, glow, Color32::from_rgba_unmultiplied(255, 80, 20, 0), Color32::from_rgba_unmultiplied(255, 80, 20, 60));
            for i in 0..60i64 {
                let h = hash(i, 97);
                let speed = 25.0 + ((h >> 8) % 60) as f64;
                let span = screen.height() as f64 + 20.0;
                let y0 = ((h >> 16) % 10_000) as f64 / 10_000.0 * span;
                let prog = (y0 + now * speed) % span;
                let y = screen.bottom() as f64 + 10.0 - prog;
                let life = (prog / span) as f32; // 0 at the bottom, 1 at the top
                let drift = ((now * 1.3 + i as f64).sin() * 12.0) as f32 + life * (((h >> 40) % 40) as f32 - 20.0);
                let x = screen.left() + ((h >> 32) % 10_000) as f32 / 10_000.0 * screen.width() + drift;
                let flick = (0.6 + 0.4 * (now * 8.0 + i as f64 * 2.1).sin()) as f32;
                let a = ((1.0 - life) * 230.0 * flick) as u8;
                let c = egui::pos2(x, y as f32);
                let r = 1.0 + ((h >> 48) % 3) as f32 * 0.6;
                p.circle_filled(c, r * 3.0, Color32::from_rgba_unmultiplied(255, 90, 20, (a as f32 * 0.18) as u8));
                p.circle_filled(c, r, Color32::from_rgba_unmultiplied(255, 200, 90, a));
            }
        }
        Backdrop::Aurora(top, bottom) => {
            gradient(&p, screen, top, bottom);
            // a few faint stars
            for i in 0..60i64 {
                let h = hash(i, 131);
                let x = screen.left() + (h % 10_000) as f32 / 10_000.0 * screen.width();
                let y = screen.top() + ((h >> 16) % 10_000) as f32 / 10_000.0 * screen.height() * 0.7;
                let tw = (0.5 + 0.5 * (now * 1.1 + i as f64).sin()) as f32;
                p.circle_filled(egui::pos2(x, y), 0.8, Color32::from_rgba_unmultiplied(220, 235, 255, (60.0 + tw * 120.0) as u8));
            }
            let ribbons = [
                (0.30f32, Color32::from_rgb(60, 255, 170)),
                (0.40f32, Color32::from_rgb(90, 200, 255)),
                (0.22f32, Color32::from_rgb(180, 120, 255)),
            ];
            for (k, (base, col)) in ribbons.iter().enumerate() {
                let mut mesh = egui::Mesh::default();
                let steps = 48u32;
                for s in 0..=steps {
                    let fx = s as f32 / steps as f32;
                    let x = screen.left() + fx * screen.width();
                    let ph = now * 0.35 * (1.0 + k as f64 * 0.3) + k as f64 * 1.9;
                    let wave = ((fx as f64 * 5.0 + ph).sin() * 0.06 + (fx as f64 * 11.0 - ph * 1.3).sin() * 0.025) as f32;
                    let cy = screen.top() + screen.height() * (base + wave);
                    let thick = screen.height() * (0.10 + 0.05 * (fx as f64 * 7.0 + ph * 0.7).sin() as f32);
                    let a = (70.0 + 50.0 * (fx as f64 * 3.0 + ph).sin() as f32) as u8;
                    let idx = mesh.vertices.len() as u32;
                    mesh.colored_vertex(egui::pos2(x, cy - thick), with_alpha(*col, 0));
                    mesh.colored_vertex(egui::pos2(x, cy), with_alpha(*col, a));
                    mesh.colored_vertex(egui::pos2(x, cy + thick * 0.35), with_alpha(*col, 0));
                    if s > 0 {
                        let p0 = idx - 3;
                        mesh.add_triangle(p0, p0 + 1, idx);
                        mesh.add_triangle(p0 + 1, idx + 1, idx);
                        mesh.add_triangle(p0 + 1, p0 + 2, idx + 1);
                        mesh.add_triangle(p0 + 2, idx + 2, idx + 1);
                    }
                }
                p.add(egui::Shape::mesh(mesh));
            }
        }
    }
}

/// The same colour with a different transparency (0 = invisible, 255 = solid).
fn with_alpha(c: Color32, a: u8) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Blends colour `a` toward `b` by `t` (0..1).
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Draws a filled heart centred at `c`, about `2*size` wide.
fn heart(p: &egui::Painter, c: egui::Pos2, size: f32, color: Color32) {
    // classic heart curve: x = 16 sin^3 t, y = 13 cos t - 5 cos 2t - 2 cos 3t - cos 4t
    let scale = size / 16.0;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, color);
    let steps = 28u32;
    for k in 0..=steps {
        let t = k as f32 / steps as f32 * std::f32::consts::TAU;
        let x = 16.0 * t.sin().powi(3);
        let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
        mesh.colored_vertex(egui::pos2(c.x + x * scale, c.y - y * scale), color);
    }
    for k in 1..=steps {
        mesh.add_triangle(0, k, k + 1);
    }
    p.add(egui::Shape::mesh(mesh));
}

/// Matrix-style rain: columns of green 0s and 1s sliding down.
fn paint_rain(p: &egui::Painter, screen: egui::Rect, now: f64) {
    let cell = 14.0f32;
    let font = FontId::monospace(12.0);
    let cols = (screen.width() / cell).ceil() as i64 + 1;
    let rows = (screen.height() / cell).ceil() as i64;
    for c in 0..cols {
        let h = hash(c, 7);
        let speed = 4.0 + (h % 9) as f64; // cells per second
        let trail = 8 + (h >> 8) as i64 % 18;
        let span = rows + trail + (h >> 16) as i64 % 20;
        let head = ((now * speed) as i64 + (h >> 24) as i64) % span;
        for k in 0..trail {
            let r = head - k;
            if r < 0 || r >= rows {
                continue;
            }
            let fade = 1.0 - k as f32 / trail as f32;
            let tick = (now * 6.0) as i64;
            let bit = if hash(c * 131 + r, if k == 0 { tick } else { tick / 5 }) & 1 == 0 { "0" } else { "1" };
            let color = if k == 0 {
                Color32::from_rgba_unmultiplied(170, 255, 190, 110)
            } else {
                Color32::from_rgba_unmultiplied(0, 255, 70, (fade * 55.0) as u8)
            };
            let pos = egui::pos2(screen.left() + c as f32 * cell + 3.0, screen.top() + r as f32 * cell);
            p.text(pos, Align2::LEFT_TOP, bit, font.clone(), color);
        }
    }
}

/// Uses a theme for this run without remembering it (for `--theme`).
pub fn use_key(key: &str) {
    if let Some(i) = THEMES.iter().position(|t| t.key.eq_ignore_ascii_case(key.trim())) {
        CURRENT.store(i, Ordering::Relaxed);
    }
}

/// Mixes a colour toward white by `amount` (0..1).
pub fn lighten(c: Color32, amount: f32) -> Color32 {
    let m = |v: u8| (v as f32 + (255.0 - v as f32) * amount).round() as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// Mixes a colour toward black by `amount` (0..1).
pub fn darken(c: Color32, amount: f32) -> Color32 {
    let m = |v: u8| (v as f32 * (1.0 - amount)).round() as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// Perceived brightness, 0..255.
pub fn luma(c: Color32) -> f32 {
    0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32
}

/// The theme's accent, darkened until white text on it is easy to read.
pub fn fill_for_white_text(c: Color32) -> Color32 {
    let mut f = darken(c, 0.25);
    while luma(f) > 105.0 {
        f = darken(f, 0.12);
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glyphs the app's built-in fonts can draw (missing ones show as a box).
    fn drawable(ctx: &egui::Context, s: &str) -> bool {
        ctx.fonts(|f| f.has_glyphs(&FontId::proportional(14.0), s))
    }

    fn ready_ctx() -> egui::Context {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |_| {});
        ctx
    }

    #[test]
    fn every_theme_icon_can_be_drawn() {
        let ctx = ready_ctx();
        let missing: Vec<String> = THEMES
            .iter()
            .filter(|t| !drawable(&ctx, t.icon))
            .map(|t| format!("{} {}", t.name, t.icon))
            .collect();
        assert!(missing.is_empty(), "icons without a glyph: {missing:?}");
    }

    /// The symbols the window's buttons use.
    #[test]
    fn ui_symbols_can_be_drawn() {
        let ctx = ready_ctx();
        let missing: Vec<&str> = ["⏵", "⏷", "☰", "⛶", "⟳", "📱", "…", "•", "›", "❓"]
            .into_iter()
            .filter(|s| !drawable(&ctx, s))
            .collect();
        assert!(missing.is_empty(), "symbols without a glyph: {missing:?}");
    }

    #[test]
    fn theme_keys_are_unique() {
        for (i, a) in THEMES.iter().enumerate() {
            for b in THEMES.iter().skip(i + 1) {
                assert_ne!(a.key, b.key);
            }
        }
    }
}
