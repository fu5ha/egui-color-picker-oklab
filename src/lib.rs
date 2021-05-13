//! Color picker widget using Oklab/Oklch color spaces.

use std::ops::RangeInclusive;

use egui::*;
use epaint::*;
use colstodian::*;

pub type PerceptualLCh = ColorAlpha<Oklch, Separate>;
pub type Asset = ColorAlpha<EncodedSrgb, Premultiplied>;

trait IntoEguiColor {
    fn into_egui(self) -> Color32;
}

impl IntoEguiColor for Asset {
    fn into_egui(self) -> Color32 {
        let enc = self.to_u8();
        Color32::from_rgba_premultiplied(enc[0], enc[1], enc[2], enc[3])
    }
}

mod cache;
use cache::Cache;

fn contrast_color(color: impl Into<Rgba>) -> Color32 {
    if color.into().intensity() < 0.5 {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

/// Number of vertices per dimension in the color sliders.
/// We need at least 6 for hues, and more for smooth 2D areas.
/// Should always be a multiple of 6 to hit the peak hues in HSV/HSL (every 60°).
const N: u32 = 6 * 6;

fn background_checkers(painter: &Painter, rect: Rect) {
    let rect = rect.shrink(0.5); // Small hack to avoid the checkers from peeking through the sides

    let mut top_color = Color32::from_gray(128);
    let mut bottom_color = Color32::from_gray(32);
    let checker_size = Vec2::splat(rect.height() / 2.0);
    let n = (rect.width() / checker_size.x).round() as u32;

    let mut mesh = Mesh::default();
    for i in 0..n {
        let x = lerp(rect.left()..=rect.right(), i as f32 / (n as f32));
        mesh.add_colored_rect(
            Rect::from_min_size(pos2(x, rect.top()), checker_size),
            top_color,
        );
        mesh.add_colored_rect(
            Rect::from_min_size(pos2(x, rect.center().y), checker_size),
            bottom_color,
        );
        std::mem::swap(&mut top_color, &mut bottom_color);
    }
    painter.add(Shape::mesh(mesh));
}

fn show_color(ui: &mut Ui, color: Color32, desired_size: Vec2) -> Response {
    let (rect, response) = ui.allocate_at_least(desired_size, Sense::hover());
    background_checkers(ui.painter(), rect);
    if true {
        let left = Rect::from_min_max(rect.left_top(), rect.center_bottom());
        let right = Rect::from_min_max(rect.center_top(), rect.right_bottom());
        ui.painter().rect_filled(left, 0.0, color);
        ui.painter().rect_filled(right, 0.0, color.to_opaque());
    } else {
        ui.painter().add(Shape::Rect {
            rect,
            corner_radius: 2.0,
            fill: color.into(),
            stroke: Stroke::new(3.0, color.to_opaque()),
        });
    }
    response
}

fn color_button(ui: &mut Ui, color: Color32) -> Response {
    let size = ui.spacing().interact_size;
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::new(WidgetType::ColorButton));
    let visuals = ui.style().interact(&response);
    let rect = rect.expand(visuals.expansion);

    background_checkers(ui.painter(), rect);

    let left_half = Rect::from_min_max(rect.left_top(), rect.center_bottom());
    let right_half = Rect::from_min_max(rect.center_top(), rect.right_bottom());
    ui.painter().rect_filled(left_half, 0.0, color);
    ui.painter().rect_filled(right_half, 0.0, color.to_opaque());

    let corner_radius = visuals.corner_radius.at_most(2.0);
    ui.painter()
        .rect_stroke(rect, corner_radius, (2.0, visuals.bg_fill)); // fill is intentional!

    response
}

fn color_slider_1d(ui: &mut Ui, value: &mut f32, range: RangeInclusive<f32>, color_at: impl Fn(f32) -> Color32) -> Response {
    #![allow(clippy::identity_op)]

    let desired_size = vec2(
        ui.spacing().slider_width,
        ui.spacing().interact_size.y * 2.0,
    );
    let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

    if let Some(mpos) = response.interact_pointer_pos() {
        *value = remap_clamp(mpos.x, rect.left()..=rect.right(), range.clone());
    }

    let visuals = ui.style().interact(&response);

    background_checkers(ui.painter(), rect); // for alpha:

    {
        // fill color:
        let mut mesh = Mesh::default();
        for i in 0..=N {
            let t = i as f32 / (N as f32);
            let color = color_at(lerp(range.clone(), t));
            let x = lerp(rect.left()..=rect.right(), t);
            mesh.colored_vertex(pos2(x, rect.top()), color);
            mesh.colored_vertex(pos2(x, rect.bottom()), color);
            if i < N {
                mesh.add_triangle(2 * i + 0, 2 * i + 1, 2 * i + 2);
                mesh.add_triangle(2 * i + 1, 2 * i + 2, 2 * i + 3);
            }
        }
        ui.painter().add(Shape::mesh(mesh));
    }

    ui.painter().rect_stroke(rect, 0.0, visuals.bg_stroke); // outline

    {
        // Show where the slider is at:
        let x = lerp(rect.left()..=rect.right(), remap_clamp(*value, range.clone(), 0.0..=1.0));
        let r = rect.height() / 4.0;
        let picked_color = color_at(*value);
        ui.painter().add(Shape::polygon(
            vec![
                pos2(x - r, rect.bottom()),
                pos2(x + r, rect.bottom()),
                pos2(x, rect.center().y),
            ],
            picked_color,
            Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
        ));
    }

    response
}

fn color_slider_2d(
    ui: &mut Ui,
    x_value: &mut f32,
    x_range: RangeInclusive<f32>,
    y_value: &mut f32,
    y_range: RangeInclusive<f32>,
    color_at: impl Fn(f32, f32) -> Color32,
) -> Response {
    let desired_size = Vec2::splat(ui.spacing().slider_width);
    let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

    if let Some(mpos) = response.interact_pointer_pos() {
        *x_value = remap_clamp(mpos.x, rect.left()..=rect.right(), x_range.clone());
        *y_value = remap_clamp(mpos.y, rect.bottom()..=rect.top(), y_range.clone());
    }

    let visuals = ui.style().interact(&response);
    let mut mesh = Mesh::default();

    for xi in 0..=N {
        for yi in 0..=N {
            let xt = xi as f32 / (N as f32);
            let yt = yi as f32 / (N as f32);
            let color = color_at(lerp(x_range.clone(), xt), lerp(y_range.clone(), yt));
            let x = lerp(rect.left()..=rect.right(), xt);
            let y = lerp(rect.bottom()..=rect.top(), yt);
            mesh.colored_vertex(pos2(x, y), color);

            if xi < N && yi < N {
                let x_offset = 1;
                let y_offset = N + 1;
                let tl = yi * y_offset + xi;
                mesh.add_triangle(tl, tl + x_offset, tl + y_offset);
                mesh.add_triangle(tl + x_offset, tl + y_offset, tl + y_offset + x_offset);
            }
        }
    }
    ui.painter().add(Shape::mesh(mesh)); // fill

    ui.painter().rect_stroke(rect, 0.0, visuals.bg_stroke); // outline

    // Show where the slider is at:
    let x = lerp(rect.left()..=rect.right(), remap_clamp(*x_value, x_range.clone(), 0.0..=1.0));
    let y = lerp(rect.bottom()..=rect.top(), remap_clamp(*y_value, y_range.clone(), 0.0..=1.0));
    let picked_color = color_at(*x_value, *y_value);
    ui.painter().add(Shape::Circle {
        center: pos2(x, y),
        radius: rect.width() / 12.0,
        fill: picked_color,
        stroke: Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
    });

    response
}

fn color_text_ui(ui: &mut Ui, color: Asset) {
    ui.horizontal(|ui| {
        let [r, g, b, a] = color.to_u8();
        ui.label(format!(
            "Encoded sRGB + Alpha (premultiplied): ({}, {}, {}, {})",
            r, g, b, a
        ));

        if ui.button("📋").on_hover_text("Click to copy").clicked() {
            ui.output().copied_text = format!("{}, {}, {}, {}", r, g, b, a);
        }
    });
}

fn color_picker_oklch_2d(ui: &mut Ui, color: &mut PerceptualLCh, col_srgba: Asset) -> bool {
    let orig_col = *color;

    color_text_ui(ui, col_srgba);

    let grid_id = "oklab_color_picker";

    crate::Grid::new(grid_id).show(ui, |ui| {
        let current_color_size = vec2(
            ui.spacing().slider_width,
            ui.spacing().interact_size.y * 2.0,
        );

        let mut opaque = *color;
        opaque.alpha = 1.0;

        color_slider_1d(ui, &mut color.alpha, 0.0..=1.0, |a| {
            let mut col = opaque;
            col.alpha = a;
            col.convert::<EncodedSrgb, Premultiplied>().into_egui()
        });
        ui.label("Alpha");
        ui.end_row();

        show_color(ui, color.convert::<EncodedSrgb, Premultiplied>().into_egui(), current_color_size);
        ui.label("Selected color");
        ui.end_row();

        ui.separator(); // TODO: fix ever-expansion
        ui.end_row();

        use core::f32::consts::PI;
        color_slider_1d(ui, &mut color.col.h, -PI..=PI, |h| {
            let mut col = opaque;
            col.col.h = h;
            col.convert::<EncodedSrgb, Premultiplied>().into_egui()
        });
        ui.label("Hue");
        ui.end_row();

        color_slider_1d(ui, &mut color.col.c,0.0..=0.5, |c| {
            let mut col = opaque;
            col.col.c = c;
            col.convert::<EncodedSrgb, Premultiplied>().into_egui()
        });
        ui.label("Chroma");
        ui.end_row();

        color_slider_1d(ui, &mut color.col.l, 0.0..=1.0, |l| {
            let mut col = opaque;
            col.col.l = l;
            col.convert::<EncodedSrgb, Premultiplied>().into_egui()
        });
        ui.label("Lightness");
        ui.end_row();

        let col = &mut color.col;
        color_slider_2d(ui, &mut col.c, 0.0..=0.5, &mut col.l, 0.0..=1.0, |c, l| {
            let mut col = opaque;
            col.col.c = c;
            col.col.l = l;
            col.convert::<EncodedSrgb, Premultiplied>().into_egui()
        });
        ui.label("Lightness / Chroma");
        ui.end_row();
    });

    if *color == orig_col {
        false
    } else {
        true
    }
}

pub fn color_edit_button_oklch(ui: &mut Ui, color: &mut PerceptualLCh) -> Response {
    let col_srgba = color.convert::<EncodedSrgb, Premultiplied>();
    let popup_id = ui.make_persistent_id("popup");
    let mut button_response = color_button(ui, col_srgba.into_egui()).on_hover_text("Click to edit color");

    if button_response.clicked() {
        ui.memory().toggle_popup(popup_id);
    }
    // TODO: make it easier to show a temporary popup that closes when you click outside it
    if ui.memory().is_popup_open(popup_id) {
        let area_response = Area::new(popup_id)
            .order(Order::Foreground)
            .default_pos(button_response.rect.max)
            .show(ui.ctx(), |ui| {
                ui.spacing_mut().slider_width = 256.0;
                Frame::popup(ui.style()).show(ui, |ui| {
                    if color_picker_oklch_2d(ui, color, col_srgba) {
                        button_response.mark_changed();
                    }
                });
            });

        if !button_response.clicked()
            && (ui.input().key_pressed(Key::Escape) || area_response.clicked_elsewhere())
        {
            ui.memory().close_popup();
        }
    }

    button_response
}

fn color_edit_button_inner(ui: &mut Ui, color: &mut Asset) -> Response {
    // To ensure we keep hue slider when `color` is gray we store the
    // full Oklch color in a cache:

    let mut oklch = ui
        .ctx()
        .memory()
        .data_temp
        .get_or_default::<Cache<[u8; 4], PerceptualLCh>>()
        .get(&color.to_u8())
        .cloned()
        .unwrap_or_else(|| color.convert());

    let response = color_edit_button_oklch(ui, &mut oklch);

    *color = oklch.convert();

    ui.ctx()
        .memory()
        .data_temp
        .get_mut_or_default::<Cache<[u8; 4], PerceptualLCh>>()
        .set(color.to_u8(), oklch);

    response
}

/// Shows a button with the given color.
/// If the user clicks the button, a full color picker is shown.
pub fn color_edit_button(ui: &mut Ui, color: &mut Color32) -> Response {
    let mut col = Asset::from_u8([color[0], color[1], color[2], color[3]]);

    let res = color_edit_button_inner(ui, &mut col);

    *color = col.into_egui();

    res
}

