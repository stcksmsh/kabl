//! Blurs drawn from a small pre-blurred texture instead of tessellated feathers: the glow of a
//! living cable and the drop shadow of a face. The texture is made once, then each use is a
//! handful of vertices.

use egui::epaint::{Mesh, Vertex};
use egui::{
    pos2, vec2, Color32, ColorImage, Context, Id, Rect, Shape, TextureHandle, TextureId,
    TextureOptions,
};

/// A white texture `w` by `h` texels whose alpha is `alpha_at(u, v)` (each -1..1 across the
/// texture, premultiplied), made once per `key`.
pub fn texture(
    ctx: &Context,
    key: &str,
    w: usize,
    h: usize,
    alpha_at: impl Fn(f32, f32) -> f32,
) -> TextureId {
    let id = Id::new(("kabl-blur-tex", key));
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return t.id();
    }
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / h as f32 * 2.0 - 1.0;
            let a = alpha_at(u, v).clamp(0.0, 1.0);
            pixels.push(Color32::from_white_alpha((a * 255.0).round() as u8));
        }
    }
    let img = ColorImage {
        size: [w, h],
        source_size: vec2(w as f32, h as f32),
        pixels,
    };
    let t = ctx.load_texture(format!("kabl-blur-{key}"), img, TextureOptions::LINEAR);
    let tid = t.id();
    ctx.data_mut(|d| d.insert_temp(id, t));
    tid
}

/// Smooth 1 -> 0 over `blur` across the edge of a shape: `d` is the signed distance outside it.
fn feather(d: f32, blur: f32) -> f32 {
    let t = (0.5 - d / blur.max(1e-3)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The drop shadow of `rect` (already offset): a rounded box of corner `radius` feathered over
/// `blur`, black at `alpha`. Sixteen vertices (a nine-slice of the blurred corner texture); `None`
/// when the rect is too small for the slices, so the caller draws it the long way.
pub fn drop_shadow(ctx: &Context, rect: Rect, radius: f32, blur: f32, alpha: u8) -> Option<Shape> {
    let pad = (blur * 0.5).ceil() + 1.0;
    let half = (pad + radius).ceil().max(2.0);
    if rect.width() < 2.0 * radius + 1.0 || rect.height() < 2.0 * radius + 1.0 {
        return None;
    }
    let n = (half * 2.0) as usize;
    let key = format!("shadow-{blur}-{radius}");
    let tex = texture(ctx, &key, n, n, |u, v| {
        // Distance outside a box whose corners are `radius` round, from the texture's centre.
        let (x, y) = (u.abs() * half, v.abs() * half);
        let (bx, by) = (half - pad - radius, half - pad - radius);
        let (qx, qy) = (x - bx, y - by);
        let d = (qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0)) - radius;
        feather(d, blur)
    });
    let o = rect.expand(pad);
    let xs = [o.left(), o.left() + half, o.right() - half, o.right()];
    let ys = [o.top(), o.top() + half, o.bottom() - half, o.bottom()];
    let us = [0.0, 0.5, 0.5, 1.0];
    let mut mesh = Mesh::with_texture(tex);
    let color = Color32::from_black_alpha(alpha);
    for j in 0..4 {
        for i in 0..4 {
            mesh.vertices.push(Vertex {
                pos: pos2(xs[i], ys[j]),
                uv: pos2(us[i], us[j]),
                color,
            });
        }
    }
    for j in 0..3u32 {
        for i in 0..3u32 {
            let a = j * 4 + i;
            mesh.indices
                .extend_from_slice(&[a, a + 1, a + 5, a, a + 5, a + 4]);
        }
    }
    Some(Shape::mesh(mesh))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feather_runs_from_opaque_inside_to_clear_outside() {
        assert_eq!(feather(-20.0, 10.0), 1.0);
        assert_eq!(feather(20.0, 10.0), 0.0);
        assert!((feather(0.0, 10.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_face_shadow_is_sixteen_vertices_and_small_rects_fall_back() {
        let ctx = Context::default();
        let r = Rect::from_min_size(pos2(10.0, 10.0), vec2(120.0, 200.0));
        let Some(Shape::Mesh(m)) = drop_shadow(&ctx, r, 6.0, 14.0, 80) else {
            panic!("expected a mesh");
        };
        assert_eq!((m.vertices.len(), m.indices.len()), (16, 54));
        let o = r.expand(8.0);
        assert_eq!(m.vertices[0].pos, o.min);
        assert_eq!(m.vertices[15].pos, o.max);
        let tiny = Rect::from_min_size(pos2(0.0, 0.0), vec2(4.0, 4.0));
        assert!(drop_shadow(&ctx, tiny, 6.0, 14.0, 80).is_none());
    }
}
