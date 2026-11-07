//! Simulink-style look: colours, system fonts and egui visuals.
//!
//! The palette follows the layout of the Simulink R2026b editor (dark-blue
//! toolstrip tab bar, light-grey toolstrip body, white canvas). The values are
//! approximations chosen by eye; nothing here is copied from MathWorks assets.
use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke};
use std::sync::Arc;

/// Toolstrip tab bar (dark blue).
pub const TAB_BAR: Color32 = Color32::from_rgb(0x0B, 0x3E, 0x70);
/// Hovered (unselected) toolstrip tab.
pub const TAB_HOVER: Color32 = Color32::from_rgb(0x1D, 0x57, 0x8F);
/// Toolstrip body and selected tab.
pub const STRIP_BG: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);
/// Separators between toolstrip sections and around panels.
pub const BORDER: Color32 = Color32::from_rgb(0xD2, 0xD2, 0xD2);
/// Section captions (FILE, SIMULATE, …).
pub const CAPTION: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x5A);
/// Default text colour.
pub const TEXT: Color32 = Color32::from_rgb(0x21, 0x21, 0x21);
/// Disabled text / icon colour.
pub const DISABLED: Color32 = Color32::from_rgb(0xA8, 0xA8, 0xA8);
/// Hover fill of toolstrip buttons.
pub const BUTTON_HOVER: Color32 = Color32::from_rgb(0xDD, 0xE8, 0xF3);
/// Pressed fill of toolstrip buttons.
pub const BUTTON_PRESSED: Color32 = Color32::from_rgb(0xC3, 0xD8, 0xEE);
/// Explorer bar / palette background.
pub const BAR_BG: Color32 = Color32::from_rgb(0xFA, 0xFA, 0xFA);
/// Status bar background.
pub const STATUS_BG: Color32 = Color32::from_rgb(0xF0, 0xF0, 0xF0);
/// Selection / accent blue.
pub const ACCENT: Color32 = Color32::from_rgb(0x00, 0x72, 0xBD);
/// Icon blue (step buttons, inspector, …).
pub const ICON_BLUE: Color32 = Color32::from_rgb(0x1F, 0x6F, 0xB5);
/// Run button green.
pub const RUN_GREEN: Color32 = Color32::from_rgb(0x2F, 0x9E, 0x44);
/// Icon orange (new/open/library accents).
pub const ICON_ORANGE: Color32 = Color32::from_rgb(0xE8, 0x9A, 0x1E);

/// Font family used on the block diagram (Helvetica/Arial-like).
pub fn diagram_family() -> FontFamily {
    FontFamily::Name(Arc::from("diagram"))
}

/// Candidate files for the UI font (first one found wins).
const UI_FONTS: &[(&str, u32)] = &[
    ("C:\\Windows\\Fonts\\segoeui.ttf", 0),
    ("/System/Library/Fonts/SFNS.ttf", 0),
    ("/System/Library/Fonts/Helvetica.ttc", 0),
    (
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        0,
    ),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
];
/// Candidate files for the diagram font.
const DIAGRAM_FONTS: &[(&str, u32)] = &[
    ("C:\\Windows\\Fonts\\arial.ttf", 0),
    ("/System/Library/Fonts/Helvetica.ttc", 0),
    ("/Library/Fonts/Arial.ttf", 0),
    (
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        0,
    ),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
];
/// Fallback fonts for Japanese / CJK text (file names, block names).
const CJK_FONTS: &[(&str, u32)] = &[
    ("C:\\Windows\\Fonts\\YuGothM.ttc", 0),
    ("C:\\Windows\\Fonts\\meiryo.ttc", 0),
    ("C:\\Windows\\Fonts\\msgothic.ttc", 0),
    ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/truetype/fonts-japanese-gothic.ttf", 0),
];

fn load_first(cands: &[(&str, u32)]) -> Option<(String, FontData)> {
    cands.iter().find_map(|(path, index)| {
        let bytes = std::fs::read(path).ok()?;
        let mut d = FontData::from_owned(bytes);
        d.index = *index;
        Some((path.to_string(), d))
    })
}

/// Builds the font definitions: system UI font, a diagram font and a CJK
/// fallback. Missing system fonts fall back to egui's bundled fonts.
pub fn font_definitions() -> (FontDefinitions, Vec<String>) {
    let mut fonts = FontDefinitions::default();
    let mut loaded = vec![];
    let bundled_prop = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut prop: Vec<String> = vec![];
    let mut diag: Vec<String> = vec![];
    if let Some((p, d)) = load_first(UI_FONTS) {
        fonts.font_data.insert("sys-ui".into(), Arc::new(d));
        prop.push("sys-ui".into());
        loaded.push(p);
    }
    if let Some((p, d)) = load_first(DIAGRAM_FONTS) {
        fonts.font_data.insert("sys-diagram".into(), Arc::new(d));
        diag.push("sys-diagram".into());
        loaded.push(p);
    }
    let cjk = load_first(CJK_FONTS).map(|(p, d)| {
        fonts.font_data.insert("sys-cjk".into(), Arc::new(d));
        loaded.push(p);
        "sys-cjk".to_string()
    });
    // Proportional: system UI font, then bundled fonts (emoji/symbols), then CJK.
    prop.extend(bundled_prop.iter().cloned());
    diag.extend(prop.iter().cloned());
    if let Some(c) = &cjk {
        prop.push(c.clone());
        diag.push(c.clone());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push(c.clone());
    }
    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(diagram_family(), diag);
    (fonts, loaded)
}

/// Light visuals tuned to the Simulink editor.
pub fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.panel_fill = STRIP_BG;
    v.window_fill = Color32::WHITE;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(2);
    v.menu_corner_radius = CornerRadius::same(2);
    v.faint_bg_color = Color32::from_rgb(0xF4, 0xF6, 0xF8);
    v.extreme_bg_color = Color32::WHITE;
    v.selection.bg_fill = Color32::from_rgb(0xCC, 0xE4, 0xF7);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.override_text_color = Some(TEXT);
    let r = CornerRadius::same(2);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xFC, 0xFC, 0xFC);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0xFC, 0xFC, 0xFC);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0xBC, 0xBC, 0xBC));
    v.widgets.hovered.weak_bg_fill = BUTTON_HOVER;
    v.widgets.hovered.bg_fill = BUTTON_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x7F, 0xB2, 0xDE));
    v.widgets.active.weak_bg_fill = BUTTON_PRESSED;
    v.widgets.active.bg_fill = BUTTON_PRESSED;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.expansion = 0.0;
    v
}

/// Applies fonts, visuals and spacing to the context. Returns the font files used.
pub fn apply(ctx: &egui::Context) -> Vec<String> {
    let (fonts, loaded) = font_definitions();
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::Theme::Light);
    ctx.set_visuals_of(egui::Theme::Light, visuals());
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
        s.spacing.interact_size.y = 20.0;
        use egui::{FontId, TextStyle};
        s.text_styles
            .insert(TextStyle::Body, FontId::proportional(13.0));
        s.text_styles
            .insert(TextStyle::Button, FontId::proportional(13.0));
        s.text_styles
            .insert(TextStyle::Small, FontId::proportional(10.5));
        s.text_styles
            .insert(TextStyle::Heading, FontId::proportional(15.0));
        s.text_styles
            .insert(TextStyle::Monospace, FontId::monospace(12.5));
    });
    loaded
}
