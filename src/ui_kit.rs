//! Web-app style theme: slate neutrals, indigo accent, cards, pills, inputs with focus rings.

use crate::rules::Safety;
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Galley, Margin, Painter, Rect,
    CornerRadius, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui,
};
use egui::epaint::Shadow;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui_phosphor::bold as ph;
use std::cell::Cell;
use std::sync::Arc;

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub const BG: Color32 = rgb(246, 247, 249);
pub const SURFACE: Color32 = Color32::WHITE;
pub const PANE: Color32 = rgb(241, 245, 249);
pub const LINE: Color32 = rgb(226, 232, 240);
pub const LINE_STRONG: Color32 = rgb(203, 213, 225);
pub const FG: Color32 = rgb(15, 23, 42);
pub const FG2: Color32 = rgb(51, 65, 85);
pub const MUT: Color32 = rgb(71, 85, 105);
pub const MUT2: Color32 = rgb(100, 116, 139);
pub const ACC: Color32 = rgb(79, 70, 229);
pub const ACC_HOVER: Color32 = rgb(67, 56, 202);
pub const ACC_BG: Color32 = rgb(238, 242, 255);
pub const SAFE: Color32 = rgb(21, 128, 61);
pub const SAFE_BG: Color32 = rgb(220, 252, 231);
pub const REVIEW: Color32 = rgb(180, 83, 9);
pub const REVIEW_BG: Color32 = rgb(254, 243, 199);
pub const DANGER: Color32 = rgb(220, 38, 38);
pub const DANGER_BG: Color32 = rgb(254, 226, 226);
pub const DARK: Color32 = rgb(15, 23, 42);
pub const DARK2: Color32 = rgb(30, 41, 59);
pub const ROW_HOVER: Color32 = rgb(248, 250, 252);
pub const ROW_SEL: Color32 = rgb(245, 247, 255);
pub const ROW_FOCUS: Color32 = rgb(238, 242, 255);
pub const TREEMAP: [Color32; 8] = [
    rgb(199, 210, 254),
    rgb(191, 219, 254),
    rgb(221, 214, 254),
    rgb(165, 243, 252),
    rgb(224, 231, 255),
    rgb(219, 234, 254),
    rgb(237, 233, 254),
    rgb(207, 250, 254),
];

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}
pub fn semi(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semi".into()))
}
pub fn body(size: f32) -> FontId {
    FontId::proportional(size)
}
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn safety_color(s: Safety) -> Color32 {
    match s {
        Safety::Safe => SAFE,
        Safety::Caution => REVIEW,
        Safety::Protected => DANGER,
    }
}

pub fn safety_bg(s: Safety) -> Color32 {
    match s {
        Safety::Safe => SAFE_BG,
        Safety::Caution => REVIEW_BG,
        Safety::Protected => DANGER_BG,
    }
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Corner radii: small for tags and checkboxes, controls for buttons/inputs, cards for surfaces.
pub const R_SM: f32 = 4.0;
pub const R_CTL: f32 = 6.0;
pub const R_CARD: f32 = 8.0;
/// Height shared by buttons, inputs, selects and segmented controls so rows line up.
pub const CTL_H: f32 = 36.0;

fn focus_ring(p: &Painter, rect: Rect, rounding: f32) {
    p.rect_stroke(rect, rounding, Stroke::new(3.0_f32, ACC.gamma_multiply(0.22)), StrokeKind::Outside);
}

// ------------------------------------------------------------------ setup

fn add_font(fonts: &mut FontDefinitions, dir: &str, key: &str, files: &[&str]) -> bool {
    for f in files {
        if let Ok(bytes) = std::fs::read(format!("{dir}{f}")) {
            fonts.font_data.insert(key.into(), Arc::new(FontData::from_owned(bytes)));
            return true;
        }
    }
    false
}

pub fn setup(ctx: &egui::Context) {
    // Segoe UI ships with Windows, so the exe stays small; egui's fonts are the fallback.
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Bold);
    let dir = format!("{}\\Fonts\\", std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()));
    let reg = add_font(&mut fonts, &dir, "ui", &["segoeui.ttf"]);
    let semi_ok = add_font(&mut fonts, &dir, "ui-semi", &["seguisb.ttf"]);
    let bold_ok = add_font(&mut fonts, &dir, "ui-bold", &["segoeuib.ttf"]);
    let mono_ok = add_font(&mut fonts, &dir, "ui-mono", &["CascadiaMono.ttf", "consola.ttf"]);
    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    if reg {
        prop.insert(0, "ui".into());
    }
    let base = prop.clone();
    let named = |key: &str, ok: bool| {
        let mut v = base.clone();
        if ok {
            v.insert(0, key.into());
        }
        v
    };
    let semi_family = named("ui-semi", semi_ok);
    let bold_family = named("ui-bold", bold_ok);
    fonts.families.insert(FontFamily::Name("semi".into()), semi_family);
    fonts.families.insert(FontFamily::Name("bold".into()), bold_family);
    if mono_ok {
        let m = fonts.families.entry(FontFamily::Monospace).or_default();
        m.insert(0, "ui-mono".into());
    }
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::light();
    // Sub-pixel binning softens glyph edges; at 100% scaling crisp text matters more than kerning.
    v.text_options.subpixel_binning = false;
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = SURFACE;
    v.faint_bg_color = PANE;
    v.window_stroke = Stroke::new(1.0_f32, LINE);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(R_CARD as u8);
    v.window_shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(38) };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(24) };
    v.selection.bg_fill = ACC.gamma_multiply(0.22);
    v.selection.stroke = Stroke::new(1.0_f32, ACC);
    v.hyperlink_color = ACC;
    v.text_cursor.stroke = Stroke::new(2.0_f32, ACC);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(R_CTL as u8);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, FG);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, FG);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, LINE_STRONG);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACC);
    ctx.set_visuals(v);

    let mut style = (*ctx.global_style()).clone();
    use egui::TextStyle;
    style.text_styles.insert(TextStyle::Body, body(15.0));
    style.text_styles.insert(TextStyle::Button, semi(14.0));
    style.text_styles.insert(TextStyle::Small, body(13.0));
    style.text_styles.insert(TextStyle::Heading, bold(22.0));
    style.text_styles.insert(TextStyle::Monospace, mono(13.0));
    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.button_padding = vec2(12.0, 6.0);
    style.spacing.scroll = egui::style::ScrollStyle::thin();
    style.spacing.menu_margin = Margin::same(6);
    style.spacing.interact_size.y = CTL_H;
    // This is an app, not a document: labels are not selectable, and no I-beam over plain text.
    style.interaction.selectable_labels = false;
    style.interaction.multi_widget_text_select = false;
    ctx.set_global_style(style);
}

// ------------------------------------------------------------------ text

pub fn galley(ui: &Ui, text: impl Into<String>, font: FontId, color: Color32, max_w: f32) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(text.into(), TextFormat::simple(font, color));
    job.wrap = TextWrapping::truncate_at_width(max_w.max(1.0));
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Single-line text inside `rect`, vertically centred, truncated with an ellipsis.
pub fn cell_text(ui: &Ui, p: &Painter, rect: Rect, text: impl Into<String>, font: FontId, color: Color32, right: bool) {
    let g = galley(ui, text, font, color, rect.width());
    let y = rect.center().y - g.size().y / 2.0;
    let x = if right { rect.right() - g.size().x } else { rect.left() };
    p.galley(pos2(x, y), g, color);
}

/// Like `cell_text`, but cuts from the left ("...\src-tauri\target"), which keeps the useful end of a path.
pub fn cell_path(ui: &Ui, p: &Painter, rect: Rect, text: &str, font: FontId, color: Color32) {
    let fits = |s: &str| ui.fonts_mut(|f| f.layout_no_wrap(s.to_string(), font.clone(), color)).size().x <= rect.width();
    if fits(text) {
        return cell_text(ui, p, rect, text, font, color, false);
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        let s: String = std::iter::once('\u{2026}').chain(chars[mid..].iter().copied()).collect();
        if fits(&s) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let s: String = std::iter::once('\u{2026}').chain(chars[lo.min(chars.len())..].iter().copied()).collect();
    cell_text(ui, p, rect, s, font, color, false);
}

/// Tinted pill ("Safe", "Review", "Protected") painted at the left of `rect`, vertically centred.
pub fn pill(ui: &Ui, p: &Painter, left_center: egui::Pos2, text: &str, fg: Color32, bg: Color32) -> f32 {
    let g = galley(ui, text, semi(13.0), fg, f32::INFINITY);
    let r = Rect::from_min_size(pos2(left_center.x, left_center.y - 11.0), vec2(g.size().x + 18.0, 22.0));
    p.rect_filled(r, R_SM, bg);
    p.galley(r.center() - g.size() / 2.0, g, fg);
    r.width()
}

pub fn safety_pill(ui: &Ui, p: &Painter, rect: Rect, s: Safety) {
    pill(ui, p, pos2(rect.left(), rect.center().y), s.label(), safety_color(s), safety_bg(s));
}

/// Inline safety pill as a widget.
pub fn badge(ui: &mut Ui, s: Safety) {
    let g = galley(ui, s.label(), semi(13.0), safety_color(s), f32::INFINITY);
    let (r, _) = ui.allocate_exact_size(vec2(g.size().x + 18.0, 22.0), Sense::hover());
    ui.painter().rect_filled(r, R_SM, safety_bg(s));
    ui.painter().galley(r.center() - g.size() / 2.0, g, safety_color(s));
}

/// Sidebar group heading, indented to line up with the nav item icons.
pub fn nav_section(ui: &mut Ui, s: &str) {
    ui.add_space(14.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    let r = Rect::from_min_max(pos2(r.left() + 12.0, r.top()), r.max);
    cell_text(ui, ui.painter(), r, s.to_uppercase(), semi(12.0), MUT2, false);
    ui.add_space(2.0);
}

/// Heading above a group of cards on a page, with optional muted text on the right.
pub fn section_title(ui: &mut Ui, title: &str, right: &str) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let p = ui.painter();
    cell_text(ui, p, r, title, semi(15.0), FG, false);
    if !right.is_empty() {
        cell_text(ui, p, r, right, body(13.0), MUT, true);
    }
    ui.add_space(6.0);
}

pub fn divider(ui: &mut Ui) {
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, LINE);
    ui.add_space(4.0);
}

pub fn kbd(ui: &mut Ui, s: &str) {
    let g = galley(ui, s, mono(12.0), FG2, f32::INFINITY);
    let (r, _) = ui.allocate_exact_size(vec2(g.size().x + 12.0, 20.0), Sense::hover());
    let p = ui.painter();
    p.rect(r, R_SM, SURFACE, Stroke::new(1.0_f32, LINE_STRONG), StrokeKind::Inside);
    p.galley(r.center() - g.size() / 2.0, g, FG2);
}

pub fn bar(p: &Painter, rect: Rect, frac: f32, fill: Color32) {
    let r = (rect.height() / 2.0).min(3.0);
    p.rect_filled(rect, r, PANE);
    let mut f = rect;
    f.set_width(rect.width() * frac.clamp(0.0, 1.0));
    if f.width() > 0.5 {
        p.rect_filled(f, r, fill);
    }
}

// ------------------------------------------------------------------ surfaces

pub fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(R_CARD as u8)
}

pub fn card<R>(ui: &mut Ui, pad: f32, f: impl FnOnce(&mut Ui) -> R) -> R {
    card_frame().inner_margin(Margin::same(pad as i8)).show(ui, f).inner
}

/// Space between cards, and between a page's sections.
pub const GAP: f32 = 16.0;

fn card_shapes(rect: Rect) -> Shape {
    Shape::Vec(vec![
        Shape::rect_filled(rect, R_CARD, SURFACE),
        Shape::rect_stroke(rect, R_CARD, Stroke::new(1.0_f32, LINE), StrokeKind::Inside),
    ])
}

/// Lays out `n` equal-width cells of the same height (the tallest one's); returns the whole row and each cell.
/// `bg` is a placeholder painted before the content, for the caller to fill once the height is known.
fn cells(ui: &mut Ui, n: usize, gap: f32, pad: f32, add: &mut dyn FnMut(&mut Ui, usize)) -> (egui::layers::ShapeIdx, Rect, Vec<Rect>) {
    let w = ui.available_width();
    let cw = ((w - gap * (n - 1) as f32) / n as f32).floor();
    let top = ui.cursor().min;
    let bg = ui.painter().add(Shape::Noop);
    let mut h: f32 = 0.0;
    for i in 0..n {
        let x = top.x + i as f32 * (cw + gap);
        let inner = Rect::from_min_size(pos2(x + pad, top.y + pad), vec2(cw - 2.0 * pad, 10_000.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_width(inner.width());
        add(&mut child, i);
        h = h.max(child.min_rect().height());
    }
    let h = h + 2.0 * pad;
    let row = Rect::from_min_size(top, vec2(w, h));
    ui.allocate_rect(row, Sense::hover());
    let cells = (0..n).map(|i| Rect::from_min_size(pos2(top.x + i as f32 * (cw + gap), top.y), vec2(cw, h))).collect();
    (bg, row, cells)
}

/// `n` cards side by side, equal width and equal height (the tallest one's), filling the row.
pub fn card_row(ui: &mut Ui, n: usize, pad: f32, mut add: impl FnMut(&mut Ui, usize)) {
    if n == 0 {
        return;
    }
    let (bg, _, cells) = cells(ui, n, GAP, pad, &mut add);
    ui.painter().set(bg, Shape::Vec(cells.into_iter().map(card_shapes).collect()));
}

/// Full-width hairline.
pub fn hairline(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, LINE);
}

/// Wraps `count` cards into rows with as many columns as fit at `min_w` each (at most `max_cols`).
pub fn card_grid(ui: &mut Ui, count: usize, min_w: f32, max_cols: usize, pad: f32, mut add: impl FnMut(&mut Ui, usize)) {
    // Fewer items than columns: stretch them across the row so it lines up with the rows below.
    let cols = (((ui.available_width() + GAP) / (min_w + GAP)).floor() as usize).clamp(1, max_cols.max(1)).min(count.max(1));
    let mut start = 0;
    while start < count {
        if start > 0 {
            ui.add_space(GAP);
        }
        // A short last row keeps the same column width as the rows above it.
        let n = cols.min(count - start);
        let w = ui.available_width();
        let row_w = if n < cols { (w + GAP) * n as f32 / cols as f32 - GAP } else { w };
        ui.allocate_ui_with_layout(vec2(row_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(row_w);
            card_row(ui, n, pad, |ui, i| add(ui, start + i));
        });
        start += n;
    }
}

/// Icon (or step number) in a soft rounded square.
pub fn icon_tile(ui: &mut Ui, icon: &str, size: f32, fg: Color32, bg: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui.painter().rect_filled(r, R_CTL, bg);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, body(size * 0.5), fg);
}

#[derive(Clone, Copy)]
pub enum Tone {
    Safe,
    Review,
    Danger,
    Info,
}

/// Tinted note box with a heading ("Why it's safe", "How to get it back").
pub fn callout(ui: &mut Ui, tone: Tone, title: &str, add: impl FnOnce(&mut Ui)) {
    let (bg, fg, icon) = match tone {
        Tone::Safe => (rgb(240, 253, 244), SAFE, ph::CHECK_CIRCLE),
        Tone::Review => (rgb(255, 251, 235), REVIEW, ph::WARNING),
        Tone::Danger => (rgb(254, 242, 242), DANGER, ph::SHIELD_WARNING),
        Tone::Info => (rgb(248, 250, 252), FG2, ph::INFO),
    };
    egui::Frame::NONE
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, lerp_color(bg, fg, 0.18)))
        .corner_radius(R_CTL as u8)
        .inner_margin(Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(format!("{icon}  {title}")).font(semi(14.0)).color(fg));
            add(ui);
        });
}

/// Label / value rows with a hairline between them.
pub fn kv(ui: &mut Ui, rows: &[(&str, String)]) {
    for (i, (k, v)) in rows.iter().enumerate() {
        if i > 0 {
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(r, 0.0, LINE);
        }
        let w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
        let p = ui.painter();
        cell_text(ui, p, r, *k, body(14.0), MUT, false);
        let vr = Rect::from_min_max(pos2(r.left() + w * 0.35, r.top()), r.max);
        cell_text(ui, p, vr, v.clone(), semi(14.0), FG, true);
    }
}

/// Label over a big number; meant to fill one cell of `card_row`.
pub fn stat(ui: &mut Ui, label: &str, value: &str, color: Color32) {
    ui.spacing_mut().item_spacing.y = 2.0;
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
    cell_text(ui, ui.painter(), r, label, body(13.0), MUT, false);
    let (r, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
    cell_text(ui, ui.painter(), r, value, bold(22.0), color, false);
}

/// A row of stat cards.
pub fn stats(ui: &mut Ui, items: &[(&str, String, Color32)]) {
    card_row(ui, items.len(), 16.0, |ui, i| {
        let (l, v, c) = &items[i];
        stat(ui, l, v, *c);
    });
}

pub fn progress(p: &Painter, rect: Rect, frac: f32) {
    bar(p, rect, frac, ACC);
}

// ------------------------------------------------------------------ controls

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Check {
    Off,
    On,
}

pub fn checkbox(ui: &mut Ui, state: Check, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), if enabled { Sense::click() } else { Sense::hover() });
    paint_checkbox(ui.painter(), rect, state, enabled, resp.hovered());
    if resp.has_focus() {
        focus_ring(ui.painter(), rect, R_SM);
    }
    resp
}

pub fn paint_checkbox(p: &Painter, rect: Rect, state: Check, enabled: bool, hovered: bool) {
    let a = if enabled { 1.0 } else { 0.35 };
    match state {
        Check::Off => {
            let s = if hovered && enabled { ACC } else { LINE_STRONG };
            p.rect(rect.shrink(0.5), R_SM, SURFACE, Stroke::new(1.5_f32, s.gamma_multiply(a)), StrokeKind::Inside);
        }
        Check::On => {
            p.rect_filled(rect, R_SM, ACC.gamma_multiply(a));
            let pts = vec![
                pos2(rect.left() + 4.4, rect.center().y + 0.3),
                pos2(rect.left() + 7.6, rect.bottom() - 4.6),
                pos2(rect.right() - 4.0, rect.top() + 4.8),
            ];
            p.add(Shape::line(pts, Stroke::new(2.0_f32, Color32::WHITE)));
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Look {
    Primary,
    Danger,
    Secondary,
    Ghost,
    /// Light-on-dark, for the bulk action bar.
    OnDark,
}

pub fn button(ui: &mut Ui, label: &str, look: Look, enabled: bool) -> Response {
    button_ex(ui, label, look, enabled, CTL_H, None)
}

pub fn button_small(ui: &mut Ui, label: &str, look: Look, enabled: bool) -> Response {
    button_ex(ui, label, look, enabled, 30.0, None)
}

pub fn button_wide(ui: &mut Ui, label: &str, look: Look, enabled: bool) -> Response {
    let w = ui.available_width();
    button_ex(ui, label, look, enabled, CTL_H, Some(w))
}

pub fn button_ex(ui: &mut Ui, label: &str, look: Look, enabled: bool, h: f32, width: Option<f32>) -> Response {
    let font = if h < 32.0 { semi(14.0) } else { semi(14.0) };
    let (fill, hover, stroke, text) = match look {
        Look::Primary => (ACC, ACC_HOVER, ACC, Color32::WHITE),
        Look::Danger => (DANGER, rgb(185, 28, 28), DANGER, Color32::WHITE),
        Look::Secondary => (SURFACE, ROW_HOVER, LINE_STRONG, FG),
        Look::Ghost => (Color32::TRANSPARENT, PANE, Color32::TRANSPARENT, FG2),
        Look::OnDark => (DARK2, rgb(51, 65, 85), rgb(71, 85, 105), Color32::WHITE),
    };
    let g = galley(ui, label, font, Color32::PLACEHOLDER, f32::INFINITY);
    let w = width.unwrap_or(g.size().x + 28.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    let hov = enabled && resp.hovered();
    let a = if enabled { 1.0 } else { 0.45 };
    let p = ui.painter();
    p.rect(rect, R_CTL, if hov { hover } else { fill }.gamma_multiply(a), Stroke::new(1.0_f32, stroke.gamma_multiply(a)), StrokeKind::Inside);
    p.galley(pos2(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0), g, text.gamma_multiply(a));
    if resp.has_focus() {
        focus_ring(p, rect, R_CTL);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Background, border and focus ring shared by inputs and selects.
fn field_shapes(rect: Rect, hovered: bool, active: bool) -> Shape {
    let border = if active { ACC } else if hovered { LINE_STRONG } else { LINE };
    let mut shapes = vec![Shape::rect_filled(rect, R_CTL, SURFACE)];
    if active {
        shapes.push(Shape::rect_stroke(rect, R_CTL, Stroke::new(3.0_f32, ACC.gamma_multiply(0.18)), StrokeKind::Outside));
    }
    shapes.push(Shape::rect_stroke(rect, R_CTL, Stroke::new(1.0_f32, border), StrokeKind::Inside));
    Shape::Vec(shapes)
}

/// Text field with an icon, placeholder, clear button and a visible focus ring.
pub fn input(ui: &mut Ui, id: &str, text: &mut String, hint: &str, icon: &str, width: f32, focus_now: bool) -> Response {
    let bg = ui.painter().add(Shape::Noop);
    let (rect, _) = ui.allocate_exact_size(vec2(width, CTL_H), Sense::hover());
    let area = ui.interact(rect, ui.id().with((id, "area")), Sense::click());
    let has_text = !text.is_empty();
    // `put` stretches the widget to fill its rect, so give it exactly one text row, centred.
    let font = body(14.0);
    let row_h = ui.fonts_mut(|f| f.row_height(&font));
    let inner = Rect::from_min_max(
        pos2(rect.left() + 36.0, rect.center().y - row_h / 2.0),
        pos2(rect.right() - if has_text { 32.0 } else { 12.0 }, rect.center().y + row_h / 2.0),
    );
    let edit = egui::TextEdit::singleline(text)
        .id_salt(id)
        .frame(egui::Frame::NONE)
        .font(font.clone())
        .text_color(FG)
        .hint_text(RichText::new(hint).font(font).color(MUT2))
        .desired_width(inner.width())
        .margin(Margin::ZERO);
    // A detached child, so placing the text edit doesn't move this ui's cursor back inside `rect`.
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    let resp = child.put(inner, edit);
    // Clicking anywhere on the box (icon, padding) focuses the text, like a real input.
    if focus_now || area.clicked() {
        resp.request_focus();
    }
    let focused = resp.has_focus();
    let hovered = area.hovered() || resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    ui.painter().set(bg, field_shapes(rect, hovered, focused));
    let p = ui.painter();
    p.text(pos2(rect.left() + 13.0, rect.center().y), Align2::LEFT_CENTER, icon, body(15.0), if focused { ACC } else { MUT });
    // The clear button only appears while it is useful, so a resting field stays quiet.
    if has_text && (hovered || focused) {
        let cr = Rect::from_center_size(pos2(rect.right() - 17.0, rect.center().y), vec2(22.0, 22.0));
        let c = ui.interact(cr, ui.id().with((id, "clear")), Sense::click());
        if c.hovered() {
            ui.painter().rect_filled(cr, R_SM, PANE);
        }
        ui.painter().text(cr.center(), Align2::CENTER_CENTER, ph::X, body(13.0), if c.hovered() { FG } else { MUT });
        if c.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Clear").clicked() {
            text.clear();
            resp.request_focus();
        }
    }
    resp
}

/// Dropdown trigger styled like `input`: icon, current value, caret. Pair it with `egui::Popup` and `option`.
pub fn select(ui: &mut Ui, icon: &str, value: &str, width: Option<f32>, enabled: bool) -> Response {
    let font = semi(14.0);
    let g = galley(ui, value, font.clone(), FG, f32::INFINITY);
    let w = width.unwrap_or(g.size().x + 36.0 + 34.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, CTL_H), if enabled { Sense::click() } else { Sense::hover() });
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let hov = enabled && resp.hovered();
    let a = if enabled { 1.0 } else { 0.45 };
    let p = ui.painter();
    p.add(field_shapes(rect, hov, open || resp.has_focus()));
    p.text(pos2(rect.left() + 13.0, rect.center().y), Align2::LEFT_CENTER, icon, body(15.0), (if open { ACC } else { MUT }).gamma_multiply(a));
    let g = galley(ui, value, font, Color32::PLACEHOLDER, rect.width() - 36.0 - 30.0);
    p.galley(pos2(rect.left() + 36.0, rect.center().y - g.size().y / 2.0), g, FG.gamma_multiply(a));
    let caret = if open { ph::CARET_UP } else { ph::CARET_DOWN };
    p.text(pos2(rect.right() - 13.0, rect.center().y), Align2::RIGHT_CENTER, caret, body(14.0), MUT.gamma_multiply(a));
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// One row in a dropdown: icon, label, muted detail on the right, and a check when selected.
pub fn option(ui: &mut Ui, icon: &str, label: &str, detail: &str, selected: bool) -> Response {
    let w = ui.available_width().max(200.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(r, R_SM, ACC_BG);
    } else if resp.hovered() {
        p.rect_filled(r, R_SM, PANE);
    }
    let fg = if selected { ACC } else { FG };
    p.text(pos2(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, icon, body(15.0), if selected { ACC } else { MUT });
    let right = if selected {
        p.text(pos2(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, ph::CHECK, body(15.0), ACC);
        30.0
    } else {
        10.0
    };
    let dw = if detail.is_empty() {
        0.0
    } else {
        let g = galley(ui, detail, body(13.0), MUT, f32::INFINITY);
        let dw = g.size().x;
        p.galley(pos2(r.right() - right - dw, r.center().y - g.size().y / 2.0), g, MUT);
        dw + 12.0
    };
    let g = galley(ui, label, if selected { semi(14.0) } else { body(14.0) }, fg, r.width() - 36.0 - right - dw);
    p.galley(pos2(r.left() + 36.0, r.center().y - g.size().y / 2.0), g, fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Small muted heading inside a dropdown.
pub fn option_heading(ui: &mut Ui, s: &str) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    let r = Rect::from_min_max(pos2(r.left() + 10.0, r.top()), r.max);
    cell_text(ui, ui.painter(), r, s.to_uppercase(), semi(12.0), MUT2, false);
}

fn segment_galleys(ui: &Ui, items: &[(String, String)]) -> Vec<(Arc<Galley>, Option<Arc<Galley>>, f32)> {
    items
        .iter()
        .map(|(l, c)| {
            let g = galley(ui, l.clone(), semi(14.0), Color32::PLACEHOLDER, f32::INFINITY);
            let cg = (!c.is_empty()).then(|| galley(ui, c.clone(), body(13.0), Color32::PLACEHOLDER, f32::INFINITY));
            let w = g.size().x + cg.as_ref().map_or(0.0, |c| c.size().x + 6.0) + 24.0;
            (g, cg, w)
        })
        .collect()
}

/// Width `segmented` will take for these items.
pub fn segmented_width(ui: &Ui, items: &[(String, String)]) -> f32 {
    segment_galleys(ui, items).iter().map(|s| s.2).sum::<f32>() + 6.0
}

/// Segmented control of (label, muted count) items; returns the clicked index.
pub fn segmented(ui: &mut Ui, items: &[(String, String)], selected: usize) -> Option<usize> {
    let gs = segment_galleys(ui, items);
    let w: f32 = gs.iter().map(|s| s.2).sum::<f32>() + 6.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, CTL_H), Sense::hover());
    ui.painter().rect(rect, R_CTL, PANE, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    let mut x = rect.left() + 3.0;
    let mut clicked = None;
    for (i, (g, cg, iw)) in gs.into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.top() + 3.0), vec2(iw, rect.height() - 6.0));
        let resp = ui.interact(r, ui.id().with(("seg", i, items.len())), Sense::click());
        let p = ui.painter();
        let on = i == selected;
        if on {
            p.rect(r, R_SM, SURFACE, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
        } else if resp.hovered() {
            p.rect_filled(r, R_SM, lerp_color(PANE, LINE, 0.5));
        }
        let col = if on { FG } else { FG2 };
        let mut tx = r.left() + 12.0;
        let gw = g.size().x;
        p.galley(pos2(tx, r.center().y - g.size().y / 2.0), g, col);
        if let Some(cg) = cg {
            tx += gw + 6.0;
            p.galley(pos2(tx, r.center().y - cg.size().y / 2.0), cg, if on { ACC } else { MUT });
        }
        if resp.has_focus() {
            focus_ring(p, r, R_SM);
        }
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            clicked = Some(i);
        }
        x += iw;
    }
    clicked
}

/// Sidebar navigation row; returns true when clicked.
pub fn nav_item(ui: &mut Ui, icon: &str, label: &str, count: &str, active: bool, enabled: bool) -> bool {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 36.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if active {
        p.rect_filled(r, R_CTL, ACC_BG);
    } else if enabled && resp.hovered() {
        p.rect_filled(r, R_CTL, PANE);
    }
    let a = if enabled { 1.0 } else { 0.4 };
    let (ic, tc, font) = if active { (ACC, ACC, semi(14.0)) } else { (MUT, FG2, body(14.0)) };
    p.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, icon, body(16.0), ic.gamma_multiply(a));
    let cw = if count.is_empty() { 0.0 } else { galley(ui, count, semi(12.0), MUT, f32::INFINITY).size().x + 14.0 };
    let room = r.width() - 50.0 - cw;
    let cut = galley(ui, label, font.clone(), FG, f32::INFINITY).size().x > room;
    let g = galley(ui, label, font, Color32::PLACEHOLDER, room);
    p.galley(pos2(r.left() + 38.0, r.center().y - g.size().y / 2.0), g, tc.gamma_multiply(a));
    if !count.is_empty() {
        let g = galley(ui, count, semi(12.0), Color32::PLACEHOLDER, f32::INFINITY);
        let br = Rect::from_min_size(pos2(r.right() - 8.0 - g.size().x - 14.0, r.center().y - 10.0), vec2(g.size().x + 14.0, 20.0));
        p.rect_filled(br, R_SM, if active { SURFACE } else { PANE });
        p.galley(br.center() - g.size() / 2.0, g, (if active { ACC } else { MUT }).gamma_multiply(a));
    }
    if resp.has_focus() {
        focus_ring(p, r, R_CTL);
    }
    let resp = if cut { resp.on_hover_text(label) } else { resp };
    enabled && resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

// ------------------------------------------------------------------ table

pub const ROW_H: f32 = 40.0;
const HEAD_H: f32 = 38.0;

pub struct Col {
    pub title: &'static str,
    /// 0 = takes the remaining width.
    pub width: f32,
    pub right: bool,
}

pub const fn col(title: &'static str, width: f32, right: bool) -> Col {
    Col { title, width, right }
}

#[derive(Default)]
pub struct TableOut {
    pub clicked: Option<usize>,
    pub double: Option<usize>,
    pub check: Option<usize>,
    pub header: Option<usize>,
}

pub struct RowState {
    /// Checkbox state and whether it can be toggled. None = no checkbox.
    pub check: Option<(Check, bool)>,
    pub focus: bool,
}

/// Virtualised table meant to sit inside a card. Column 0 is the checkbox column when `has_check`.
#[allow(clippy::too_many_arguments)]
pub fn table(
    ui: &mut Ui,
    id: &str,
    cols: &[Col],
    has_check: bool,
    rows: usize,
    sort: Option<usize>,
    scroll_to: Option<usize>,
    view: &Cell<(f32, f32)>,
    state: &dyn Fn(usize) -> RowState,
    cell: &mut dyn FnMut(&Ui, &Painter, usize, usize, Rect),
) -> TableOut {
    let mut out = TableOut::default();
    let width = ui.available_width();
    // When the window is narrow, shrink the fixed columns (not the checkbox) so the flexible
    // column keeps at least 200px and nothing is pushed off the right edge.
    // Right-aligned (numeric) columns keep their width too: a cut-off size is useless.
    let keep = |i: usize, c: &Col| (has_check && i == 0) || c.right;
    let kept: f32 = cols.iter().enumerate().filter(|(i, c)| keep(*i, c)).map(|(_, c)| c.width).sum();
    let fixed: f32 = cols.iter().enumerate().filter(|(i, c)| !keep(*i, c)).map(|(_, c)| c.width).sum();
    let room = (width - kept - 200.0).max(0.0);
    let scale = if fixed > room { (room / fixed).max(0.55) } else { 1.0 };
    let flex = (width - kept - fixed * scale).max(120.0);
    let xs: Vec<(f32, f32)> = cols
        .iter()
        .enumerate()
        .scan(0.0, |x, (i, c)| {
            let w = match c.width {
                0.0 => flex,
                w if keep(i, c) => w,
                w => w * scale,
            };
            let r = (*x, w);
            *x += w;
            Some(r)
        })
        .collect();
    let pad = 14.0;

    let (head, _) = ui.allocate_exact_size(vec2(width, HEAD_H), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(head, CornerRadius { nw: R_CARD as u8, ne: R_CARD as u8, sw: 0, se: 0 }, ROW_HOVER);
    p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0_f32, LINE));
    for (i, (c, &(x, w))) in cols.iter().zip(&xs).enumerate() {
        if c.title.is_empty() {
            continue;
        }
        let r = Rect::from_min_size(pos2(head.left() + x, head.top()), vec2(w, HEAD_H)).shrink2(vec2(pad, 0.0));
        let resp = ui.interact(r, ui.id().with((id, "h", i)), Sense::click());
        let arrow = if sort == Some(i) { format!("  {}", ph::ARROW_DOWN) } else { String::new() };
        let col = if sort == Some(i) { FG } else if resp.hovered() { FG2 } else { MUT };
        cell_text(ui, &p, r, format!("{}{arrow}", c.title.to_uppercase()), semi(12.0), col, c.right);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            out.header = Some(i);
        }
    }

    let (off, h) = view.get();
    let mut sa = egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]);
    if let Some(pos) = scroll_to {
        let top = pos as f32 * ROW_H;
        let target = if top < off { top } else if top + ROW_H > off + h { top + ROW_H - h } else { off };
        sa = sa.vertical_scroll_offset(target.max(0.0));
    }
    // Rows are clickable but not Tab stops, so Tab moves between real controls.
    let row_sense = Sense::CLICK;
    let res = sa.show_rows(ui, ROW_H, rows, |ui, range| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for i in range {
            let (r, resp) = ui.allocate_exact_size(vec2(width, ROW_H), row_sense);
            let st = state(i);
            let selected = matches!(st.check, Some((Check::On, _)));
            let bg = if st.focus {
                ROW_FOCUS
            } else if selected {
                ROW_SEL
            } else if resp.hovered() {
                ROW_HOVER
            } else {
                Color32::TRANSPARENT
            };
            let p = ui.painter().clone();
            p.rect_filled(r, 0.0, bg);
            if st.focus {
                p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 0.0, ACC);
            }
            p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0_f32, rgb(241, 245, 249)));
            let mut check_clicked = false;
            if has_check {
                if let Some((c, enabled)) = st.check {
                    let cb = Rect::from_center_size(pos2(r.left() + xs[0].1 / 2.0 + 2.0, r.center().y), vec2(18.0, 18.0));
                    let cr = ui.interact(cb.expand(8.0), ui.id().with((id, "c", i)), if enabled { row_sense } else { Sense::hover() });
                    paint_checkbox(&p, cb, c, enabled, cr.hovered());
                    if cr.clicked() && enabled {
                        out.check = Some(i);
                        check_clicked = true;
                    }
                }
            }
            for (ci, &(x, w)) in xs.iter().enumerate() {
                if has_check && ci == 0 {
                    continue;
                }
                let cr = Rect::from_min_size(pos2(r.left() + x, r.top()), vec2(w, ROW_H)).shrink2(vec2(pad, 0.0));
                cell(ui, &p, i, ci, cr);
            }
            if !check_clicked {
                if resp.double_clicked() {
                    out.double = Some(i);
                } else if resp.clicked() {
                    out.clicked = Some(i);
                }
            }
        }
    });
    view.set((res.state.offset.y, res.inner_rect.height()));
    out
}

// ------------------------------------------------------------------ treemap

/// Squarified treemap layout. `values` must be positive and sorted descending.
pub fn squarify(values: &[f64], rect: Rect) -> Vec<Rect> {
    let total: f64 = values.iter().sum();
    if total <= 0.0 || rect.area() <= 0.0 {
        return vec![Rect::NOTHING; values.len()];
    }
    let area = rect.width() as f64 * rect.height() as f64;
    let scaled: Vec<f64> = values.iter().map(|v| v / total * area).collect();
    let worst = |row: &[f64], side: f64| {
        let s: f64 = row.iter().sum();
        let mx = row.iter().cloned().fold(f64::MIN, f64::max);
        let mn = row.iter().cloned().fold(f64::MAX, f64::min);
        let (s2, w2) = (s * s, side * side);
        f64::max(w2 * mx / s2, s2 / (w2 * mn))
    };
    let mut out = Vec::with_capacity(values.len());
    let mut free = rect;
    let mut i = 0;
    while i < scaled.len() {
        let side = free.width().min(free.height()) as f64;
        let mut j = i + 1;
        while j < scaled.len() && worst(&scaled[i..=j], side) <= worst(&scaled[i..j], side) {
            j += 1;
        }
        let row = &scaled[i..j];
        let sum: f64 = row.iter().sum();
        if free.width() >= free.height() {
            let w = (sum / free.height() as f64) as f32;
            let mut y = free.top();
            for v in row {
                let h = (v / sum) as f32 * free.height();
                out.push(Rect::from_min_size(pos2(free.left(), y), vec2(w, h)));
                y += h;
            }
            free.min.x += w;
        } else {
            let h = (sum / free.width() as f64) as f32;
            let mut x = free.left();
            for v in row {
                let w = (v / sum) as f32 * free.width();
                out.push(Rect::from_min_size(pos2(x, free.top()), vec2(w, h)));
                x += w;
            }
            free.min.y += h;
        }
        i = j;
    }
    out
}

/// Small treemap of labelled values; returns the index of a clicked block.
pub fn treemap(ui: &mut Ui, rect: Rect, items: &[(String, u64)]) -> Option<usize> {
    let vals: Vec<f64> = items.iter().map(|i| i.1 as f64).collect();
    let rects = squarify(&vals, rect);
    let p = ui.painter().clone();
    let mut clicked = None;
    for (i, ((name, size), r)) in items.iter().zip(rects).enumerate() {
        let r = r.shrink(1.5);
        if r.width() < 2.0 || r.height() < 2.0 {
            continue;
        }
        let resp = ui.interact(r, ui.id().with(("tm", i)), Sense::click());
        let base = TREEMAP[i % TREEMAP.len()];
        p.rect_filled(r, 6.0, if resp.hovered() { lerp_color(base, ACC, 0.18) } else { base });
        if r.width() > 46.0 && r.height() > 22.0 {
            let g = galley(ui, name.clone(), semi(12.0), FG, r.width() - 12.0);
            p.galley(r.min + vec2(7.0, 5.0), g, FG);
        }
        if resp.clicked() {
            clicked = Some(i);
        }
        resp.on_hover_text(format!("{name}\n{}", crate::util::fmt_size(*size)));
    }
    clicked
}

pub fn spinner_text(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(16.0).color(ACC));
        ui.label(RichText::new(text).color(MUT));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squarify_fills_the_rect() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 200.0));
        let vals = [301.0, 36.2, 29.3, 21.4, 9.5, 8.4, 8.2];
        let out = squarify(&vals, r);
        assert_eq!(out.len(), vals.len());
        let area: f32 = out.iter().map(|r| r.area()).sum();
        assert!((area - r.area()).abs() < 1.0, "{area}");
        for b in &out {
            assert!(r.expand(0.01).contains_rect(*b), "{b:?}");
        }
    }
}
