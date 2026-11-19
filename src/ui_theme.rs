use eframe::egui::{
    self, Color32, CornerRadius, Frame, Margin, Pos2, Rect, Shadow, Stroke, StrokeKind,
    Vec2 as EguiVec2,
};

// ============================================================================
// OBSIDIAN GLASS / AEROSPACE TELEMETRY DESIGN SYSTEM
// ============================================================================

pub const BG_CANVAS: Color32 = Color32::from_rgb(9, 12, 18);
pub const BG_PANEL: Color32 = Color32::from_rgb(13, 17, 26);
pub const BG_CARD: Color32 = Color32::from_rgb(19, 25, 37);
pub const BG_WIDGET: Color32 = Color32::from_rgb(24, 32, 47);
pub const BG_WIDGET_HOVER: Color32 = Color32::from_rgb(33, 44, 65);
pub const BG_WIDGET_ACTIVE: Color32 = Color32::from_rgb(41, 56, 82);

pub const BORDER_HAIRLINE: Color32 = Color32::from_rgb(38, 49, 70);
pub const BORDER_GLASS: Color32 = Color32::from_rgb(52, 68, 96);

pub const ACCENT_CYAN: Color32 = Color32::from_rgb(56, 189, 248);
pub const ACCENT_SKY_DIM: Color32 = Color32::from_rgb(22, 60, 90);
pub const ACCENT_EMERALD: Color32 = Color32::from_rgb(52, 211, 153);
pub const ACCENT_AMBER: Color32 = Color32::from_rgb(251, 191, 36);
pub const ACCENT_ROSE: Color32 = Color32::from_rgb(248, 113, 113);
pub const ACCENT_VIOLET: Color32 = Color32::from_rgb(167, 139, 250);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(232, 240, 250);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(168, 184, 208);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(112, 130, 158);

#[inline]
pub fn bg_glass() -> Color32 {
    Color32::from_rgba_unmultiplied(12, 17, 27, 232)
}

pub fn configure_modern_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.spacing.item_spacing = EguiVec2::new(7.0, 6.0);
    style.spacing.button_padding = EguiVec2::new(9.0, 5.0);
    style.spacing.window_margin = Margin::same(12);
    style.spacing.indent = 14.0;
    style.spacing.slider_width = 145.0;
    style.spacing.combo_height = 280.0;
    style.spacing.scroll = egui::style::ScrollStyle {
        floating: true,
        bar_width: 6.0,
        handle_min_length: 22.0,
        ..Default::default()
    };

    let mut v = egui::Visuals::dark();
    v.dark_mode = true;
    v.panel_fill = BG_PANEL;
    v.window_fill = Color32::from_rgb(16, 21, 32);
    v.extreme_bg_color = Color32::from_rgb(8, 11, 17);
    v.faint_bg_color = BG_CARD;
    v.code_bg_color = Color32::from_rgb(12, 16, 24);

    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_stroke = Stroke::new(1.0_f32, BORDER_GLASS);
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 140),
    };
    v.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 130),
    };

    v.selection.bg_fill = Color32::from_rgb(26, 74, 112);
    v.selection.stroke = Stroke::new(1.2_f32, ACCENT_CYAN);
    v.hyperlink_color = ACCENT_CYAN;

    let r_sm = CornerRadius::same(6);
    v.widgets.noninteractive.bg_fill = BG_CARD;
    v.widgets.noninteractive.weak_bg_fill = BG_CARD;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_HAIRLINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_SECONDARY);
    v.widgets.noninteractive.corner_radius = r_sm;

    v.widgets.inactive.bg_fill = BG_WIDGET;
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(20, 27, 40);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_HAIRLINE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    v.widgets.inactive.corner_radius = r_sm;

    v.widgets.hovered.bg_fill = BG_WIDGET_HOVER;
    v.widgets.hovered.weak_bg_fill = BG_WIDGET_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT_CYAN);
    v.widgets.hovered.fg_stroke = Stroke::new(1.3_f32, Color32::WHITE);
    v.widgets.hovered.corner_radius = r_sm;
    v.widgets.hovered.expansion = 0.5;

    v.widgets.active.bg_fill = BG_WIDGET_ACTIVE;
    v.widgets.active.weak_bg_fill = BG_WIDGET_ACTIVE;
    v.widgets.active.bg_stroke = Stroke::new(1.3_f32, ACCENT_CYAN);
    v.widgets.active.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
    v.widgets.active.corner_radius = r_sm;

    v.widgets.open.bg_fill = Color32::from_rgb(26, 36, 54);
    v.widgets.open.weak_bg_fill = Color32::from_rgb(26, 36, 54);
    v.widgets.open.bg_stroke = Stroke::new(1.1_f32, ACCENT_CYAN);
    v.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.widgets.open.corner_radius = r_sm;

    v.slider_trailing_fill = true;

    style.visuals = v;
    ctx.set_style(style);
}

pub fn card_frame() -> Frame {
    Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0_f32, BORDER_HAIRLINE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 7))
}

pub fn studio_card(
    ui: &mut egui::Ui,
    title: &str,
    accent: Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        section_header(ui, title, accent);
        ui.add_space(2.0);
        add_contents(ui);
    });
    ui.add_space(4.0);
}

pub fn section_header(ui: &mut egui::Ui, title: &str, accent: Color32) {
    ui.horizontal(|ui| {
        let (dot_r, _) = ui.allocate_exact_size(EguiVec2::new(3.0, 12.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(dot_r, CornerRadius::same(2), accent);
        ui.label(
            egui::RichText::new(title)
                .strong()
                .size(11.0)
                .color(TEXT_PRIMARY),
        );
    });
}

pub fn pill_badge(ui: &mut egui::Ui, text: &str, fg: Color32, bg: Color32) {
    Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, fg.gamma_multiply(0.45)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).strong().monospace().size(10.0).color(fg));
        });
}

pub fn keycap_badge(ui: &mut egui::Ui, key_text: &str) {
    Frame::new()
        .fill(Color32::from_rgb(24, 32, 48))
        .stroke(Stroke::new(1.0_f32, BORDER_GLASS))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.label(
                egui::RichText::new(key_text)
                    .monospace()
                    .strong()
                    .size(10.5)
                    .color(ACCENT_AMBER),
            );
        });
}

/// Shared frosted dark-glass panel chrome for all floating HUD cards in the canvas & CAD overlays.
pub fn draw_glass_panel(painter: &egui::Painter, rect: Rect, border_col: Color32, rounding: u8) {
    let shadow_r = rect.translate(EguiVec2::new(0.0, 3.0)).expand(2.0);
    painter.rect_filled(
        shadow_r,
        CornerRadius::same(rounding + 2),
        Color32::from_rgba_unmultiplied(0, 0, 0, 95),
    );
    painter.rect_filled(rect, CornerRadius::same(rounding), bg_glass());
    painter.rect_stroke(
        rect,
        CornerRadius::same(rounding),
        Stroke::new(1.0_f32, border_col),
        StrokeKind::Outside,
    );
    // Subtle top specular hairline accent
    let pad = (rounding as f32).max(6.0);
    if rect.width() > 2.0 * pad + 8.0 {
        painter.line_segment(
            [
                Pos2::new(rect.min.x + pad, rect.min.y + 0.5),
                Pos2::new(rect.max.x - pad, rect.min.y + 0.5),
            ],
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 26)),
        );
    }
}
