//! Petri Pop design tokens.
//!
//! Transcribed from the approved mockups in `docs/New Design/Antidote
//! Redesign.dc.html` (the design-token sheet at the bottom of that page is
//! the source of truth). Everything visual — colors, type scale, spacing,
//! radii, strokes — reads from here so the app matches the mockups from a
//! single place.

use std::sync::Arc;

use agg_gui::text::Font;
use agg_gui::Color;

// ---------------------------------------------------------------------------
// Canvas geometry
// ---------------------------------------------------------------------------

/// Fixed virtual canvas the entire app is authored at. Scaled uniformly to
/// the window; never reflows.
pub const APP_W: f64 = 1280.0;
pub const APP_H: f64 = 720.0;

/// Width of each HUD rail flanking the playfield.
pub const RAIL_W: f64 = 120.0;

/// Playfield panel (dish + grid) between the rails.
pub const PLAYFIELD_X: f64 = RAIL_W;
pub const PLAYFIELD_W: f64 = APP_W - 2.0 * RAIL_W; // 1040
pub const PLAYFIELD_H: f64 = APP_H; // 720

/// The arena stroke is inset this far into the playfield panel; the live
/// physics area is the inset rect (corner rounding is cosmetic only).
pub const ARENA_INSET: f64 = 12.0;
pub const ARENA_RADIUS: f64 = 20.0;

/// Background grid cell size on the playfield.
pub const GRID_CELL: f64 = 40.0;

// ---------------------------------------------------------------------------
// Color tokens
// ---------------------------------------------------------------------------

/// Canvas background (also the letterbox bars outside the 1280×720 canvas).
pub const INK_900: Color = Color::from_rgb8(15, 11, 26);
/// Playfield dish.
pub const INK_800: Color = Color::from_rgb8(20, 15, 36);
/// Rails and overlay panels.
pub const INK_700: Color = Color::from_rgb8(28, 21, 48);
/// Raised surfaces: buttons, chips, meter track.
pub const INK_600: Color = Color::from_rgb8(38, 29, 64);
/// Hard-shadow offsets under raised surfaces (solid, no blur).
pub const EDGE_950: Color = Color::from_rgb8(8, 5, 16);

/// Antidote, primary actions, bubbles.
pub const LIME_500: Color = Color::from_rgb8(178, 255, 66);
/// Primary button hard edge.
pub const LIME_700: Color = Color::from_rgb8(98, 178, 22);
/// Primary button hover fill.
pub const LIME_400: Color = Color::from_rgb8(198, 255, 110);
/// Primary button pressed fill.
pub const LIME_600: Color = Color::from_rgb8(150, 225, 48);
/// Bubble stroke.
pub const LIME_STROKE: Color = Color::from_rgba8(210, 255, 140, 217);

/// Viruses, danger, low meter.
pub const CORAL_500: Color = Color::from_rgb8(255, 92, 72);
/// Virus gradient center highlight.
pub const CORAL_300: Color = Color::from_rgb8(255, 140, 110);
/// Virus gradient rim.
pub const CORAL_800: Color = Color::from_rgb8(150, 28, 40);

/// Antidote meter mid band (34–66%).
pub const AMBER_500: Color = Color::from_rgb8(255, 196, 54);
/// Best score, celebration gold.
pub const GOLD_400: Color = Color::from_rgb8(255, 206, 84);
/// Arena stroke (@0.35), grid (@0.06), confetti.
pub const VIOLET_400: Color = Color::from_rgb8(158, 120, 255);

/// Primary text and icons.
pub const TEXT_HI: Color = Color::from_rgb8(240, 236, 255);
/// Body / secondary text.
pub const TEXT_MID: Color = Color::from_rgb8(178, 168, 210);
/// Labels, captions.
pub const TEXT_LOW: Color = Color::from_rgb8(122, 112, 152);

/// 1px strokes and dividers.
pub const HAIRLINE: Color = Color::from_rgba8(233, 226, 255, 26);

/// Overlay scrims. Standard 0.78; first-run hints 0.45; game over 0.82.
pub const SCRIM: Color = Color::from_rgba8(10, 7, 20, 199);
pub const SCRIM_LIGHT: Color = Color::from_rgba8(10, 7, 20, 115);
pub const SCRIM_HEAVY: Color = Color::from_rgba8(10, 7, 20, 209);

/// Arena border: violet-400 at 0.35 alpha, 2px.
pub const ARENA_STROKE: Color = Color::rgba(158.0 / 255.0, 120.0 / 255.0, 1.0, 0.35);
/// Playfield grid lines: violet tint at 0.06 alpha, 1px.
pub const GRID_LINE: Color = Color::rgba(180.0 / 255.0, 150.0 / 255.0, 1.0, 0.06);

/// Antidote meter fill by value `t` in `0..=1` (thresholds at 34% / 67%).
pub fn meter_color(t: f32) -> Color {
    if t < 0.34 {
        CORAL_500
    } else if t < 0.67 {
        AMBER_500
    } else {
        LIME_500
    }
}

// ---------------------------------------------------------------------------
// Spacing · radii · strokes
// ---------------------------------------------------------------------------

/// Safe inset for interactive controls from canvas edges.
pub const SAFE_INSET: f64 = 24.0;

pub const RADIUS_BADGE: f64 = 8.0;
pub const RADIUS_CHIP: f64 = 14.0;
pub const RADIUS_BUTTON: f64 = 16.0;
pub const RADIUS_CARD: f64 = 20.0;
pub const RADIUS_PANEL: f64 = 24.0;

/// Square rail buttons (pause / fullscreen / mute).
pub const RAIL_BUTTON: f64 = 56.0;
/// Hard-shadow drop for buttons/chips (solid rect offset straight down).
pub const SHADOW_DROP: f64 = 4.0;
/// Hard-shadow drop for primary buttons and large panels.
pub const SHADOW_DROP_LG: f64 = 6.0;

// ---------------------------------------------------------------------------
// Type scale (Exo 2)
// ---------------------------------------------------------------------------

pub const FS_LOGO: f64 = 96.0; // 800 italic, ls -2
pub const FS_OVERLAY_TITLE: f64 = 52.0; // 800 italic (48–56 by screen)
pub const FS_GAMEOVER_SCORE: f64 = 72.0; // 800 tabular
pub const FS_LEVEL_NUM: f64 = 40.0; // 800
pub const FS_SCORE: f64 = 30.0; // 800 tabular
pub const FS_BUTTON: f64 = 24.0; // 800 caps, ls 2
pub const FS_BUTTON_SECONDARY: f64 = 19.0; // 800 caps, ls 1.5
pub const FS_BEST: f64 = 20.0; // 800 tabular
pub const FS_BODY: f64 = 20.0; // 500, lh 1.6
pub const FS_CHIP: f64 = 17.0; // 700 caps, ls 1
pub const FS_CAPTION: f64 = 15.0; // 600
pub const FS_LABEL: f64 = 14.0; // 700 caps, ls 2.5

/// Letter-spacing as a fraction of em (maps to
/// `font_settings::current_interval`). The mockups give letter-spacing in
/// px at a given font size; fraction = px / size.
pub const LS_LOGO: f64 = -2.0 / 96.0;
pub const LS_BUTTON: f64 = 2.0 / 24.0;
pub const LS_LABEL: f64 = 2.5 / 14.0;

// ---------------------------------------------------------------------------
// Fonts
// ---------------------------------------------------------------------------

const EXO2_MEDIUM: &[u8] = include_bytes!("../../assets/Exo2-Medium.ttf");
const EXO2_SEMIBOLD: &[u8] = include_bytes!("../../assets/Exo2-SemiBold.ttf");
const EXO2_BOLD: &[u8] = include_bytes!("../../assets/Exo2-Bold.ttf");
const EXO2_EXTRABOLD: &[u8] = include_bytes!("../../assets/Exo2-ExtraBold.ttf");
const EXO2_EXTRABOLD_ITALIC: &[u8] = include_bytes!("../../assets/Exo2-ExtraBoldItalic.ttf");

/// The five Exo 2 static instances the design uses. Latin-subset TTFs
/// (~37 KB each) bundled into the binary; SIL OFL (`assets/Exo2-OFL.txt`).
#[derive(Clone)]
pub struct Fonts {
    /// Weight 500 — body text.
    pub medium: Arc<Font>,
    /// Weight 600 — captions, hints, footnotes.
    pub semibold: Arc<Font>,
    /// Weight 700 — labels, chips.
    pub bold: Arc<Font>,
    /// Weight 800 — titles, numbers, buttons.
    pub extrabold: Arc<Font>,
    /// Weight 800 italic — logo, overlay titles, celebration lines.
    pub extrabold_italic: Arc<Font>,
}

impl Fonts {
    pub fn load() -> Self {
        let face = |bytes: &'static [u8], name: &str| {
            Arc::new(Font::from_slice(bytes).unwrap_or_else(|_| panic!("bundled font {name}")))
        };
        Self {
            medium: face(EXO2_MEDIUM, "Exo2-Medium"),
            semibold: face(EXO2_SEMIBOLD, "Exo2-SemiBold"),
            bold: face(EXO2_BOLD, "Exo2-Bold"),
            extrabold: face(EXO2_EXTRABOLD, "Exo2-ExtraBold"),
            extrabold_italic: face(EXO2_EXTRABOLD_ITALIC, "Exo2-ExtraBoldItalic"),
        }
    }
}
