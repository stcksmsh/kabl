//! A hovered scroll region owns its wheel even at its boundary. Ancestors get no residual delta.
use egui::{Id, ScrollArea, Ui, Vec2};

fn claim_id() -> Id {
    Id::new("kabl-wheel-owner")
}
pub(crate) fn begin(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(claim_id(), false));
}
pub(crate) fn claimed(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(claim_id()).unwrap_or(false))
}
pub(crate) trait OwnedScroll {
    fn show_owned<R>(
        self,
        ui: &mut Ui,
        contents: impl FnOnce(&mut Ui) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R>;
}
impl OwnedScroll for ScrollArea {
    fn show_owned<R>(
        self,
        ui: &mut Ui,
        contents: impl FnOnce(&mut Ui) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R> {
        let response = ui.scope(|ui| self.show(ui, contents));
        // The allocated container includes its scrollbar, without stealing surrounding padding.
        let rect = response.response.rect;
        if ui.is_enabled() && ui.rect_contains_pointer(rect.intersect(ui.clip_rect())) {
            ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
            ui.ctx().data_mut(|d| d.insert_temp(claim_id(), true));
        }
        response.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2, Event, MouseWheelUnit, RawInput, Rect};

    #[test]
    fn nested_scroll_owns_motion_and_boundaries_parent_owns_outside() {
        let ctx = egui::Context::default();
        let mut time = 0.;
        let mut frame =
            |pointer, delta: f32, inner_offset: Option<f32>, parent_offset: Option<f32>| {
                time += 0.1;
                begin(&ctx);
                let mut rect = Rect::NOTHING;
                let mut offsets = (0., 0.);
                let _ = ctx.run_ui(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(800., 600.))),
                        time: Some(time),
                        events: vec![
                            Event::PointerMoved(pointer),
                            Event::MouseWheel {
                                unit: MouseWheelUnit::Point,
                                phase: egui::TouchPhase::Move,
                                delta: vec2(0., delta),
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            let mut parent = ScrollArea::vertical()
                                .id_salt("parent")
                                .max_height(300.)
                                .animated(false);
                            if let Some(y) = parent_offset {
                                parent = parent.vertical_scroll_offset(y);
                            }
                            let output = parent.show_owned(ui, |ui| {
                                let mut child = ScrollArea::vertical()
                                    .id_salt("child")
                                    .max_height(100.)
                                    .animated(false);
                                if let Some(y) = inner_offset {
                                    child = child.vertical_scroll_offset(y);
                                }
                                let output = child.show_owned(ui, |ui| {
                                    ui.allocate_space(vec2(200., 600.));
                                });
                                rect = output.inner_rect;
                                offsets.0 = output.state.offset.y;
                                ui.allocate_space(vec2(200., 600.));
                            });
                            offsets.1 = output.state.offset.y;
                        });
                    },
                );
                (rect, offsets)
            };
        let (rect, _) = frame(pos2(20., 20.), 0., None, None);
        frame(rect.left_top() + vec2(20., 20.), 0., None, None);
        let (_, (inner, parent)) = frame(rect.left_top() + vec2(20., 20.), -80., None, None);
        assert!(inner > 0.);
        assert_eq!(parent, 0.);
        // At either child boundary the same gesture stays owned, including with a moved parent.
        let (_, (inner, parent)) = frame(rect.left_top() + vec2(20., 20.), -80., Some(500.), None);
        assert_eq!((inner, parent), (500., 0.));
        let (rect, _) = frame(rect.left_top() + vec2(20., 20.), 0., Some(0.), Some(20.));
        frame(rect.left_top() + vec2(20., 20.), 0., None, None);
        let (_, (inner, parent)) = frame(rect.left_top() + vec2(20., 20.), 80., None, None);
        assert_eq!((inner, parent), (0., 20.));
        let outside = pos2(rect.left() + 20., rect.bottom() + 50.);
        frame(outside, 0., None, None);
        let (_, (inner, parent)) = frame(outside, -80., None, None);
        assert_eq!(inner, 0.);
        assert!(parent > 20.);
    }
}
