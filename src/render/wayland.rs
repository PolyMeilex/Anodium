#![allow(clippy::too_many_arguments)]

use std::sync::Mutex;

use smithay::{
    backend::SwapBuffersError,
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::{get_role, with_states},
        seat::CursorImageAttributes,
        shell::wlr_layer::Layer,
    },
};

use crate::{render::renderer::RenderFrame, state::Anodium};

pub fn draw_cursor(
    frame: &mut RenderFrame,
    surface: &wl_surface::WlSurface,
    location: Point<i32, Logical>,
    output_scale: f64,
) -> Result<(), SwapBuffersError> {
    let ret = with_states(surface, |states| {
        Some(
            states
                .data_map
                .get::<Mutex<CursorImageAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .hotspot,
        )
    })
    .unwrap_or(None);
    let delta = match ret {
        Some(h) => h,
        None => {
            warn!(
                "Trying to display as a cursor a surface that does not have the CursorImage role."
            );
            (0, 0).into()
        }
    };
    draw_surface_tree(frame, surface, location - delta, output_scale)
}

pub fn draw_surface_tree(
    frame: &mut RenderFrame,
    root: &wl_surface::WlSurface,
    location: Point<i32, Logical>,
    scale: f64,
) -> Result<(), SwapBuffersError> {
    let renderer = &mut *frame.renderer;
    let frame = &mut *frame.frame;

    smithay::backend::renderer::utils::draw_surface_tree(
        renderer,
        frame,
        root,
        scale,
        location,
        &[Rectangle::from_loc_and_size((0, 0), (i32::MAX, i32::MAX))],
        &slog_scope::logger(),
    )
    .map_err(SwapBuffersError::from)
}

impl Anodium {
    pub fn draw_windows(
        &self,
        frame: &mut RenderFrame,
        output_rect: Rectangle<i32, Logical>,
        output_scale: f64,
    ) -> Result<(), SwapBuffersError> {
        // redraw the frame, in a simple but inneficient way
        for workspace in self.visible_workspaces() {
            workspace.draw_windows(frame, output_rect, output_scale)?;
        }

        if let Some(window) = self.grabed_window.as_ref() {
            // skip windows that do not overlap with a given output
            if output_rect.overlaps(window.bbox_in_comp_space()) {
                window.render(frame, output_rect.loc, output_scale);
            }
        }

        Ok(())
    }

    pub fn draw_layers(
        &self,
        frame: &mut RenderFrame,
        layer: Layer,
        output_rect: Rectangle<i32, Logical>,
        output_scale: f64,
    ) -> Result<(), SwapBuffersError> {
        let mut result = Ok(());

        for output in self.output_map.iter() {
            output
                .layer_map()
                .with_layers_from_bottom_to_top(&layer, |layer_surface| {
                    // skip layers that do not overlap with a given output
                    if !output_rect.overlaps(layer_surface.bbox()) {
                        return;
                    }

                    let mut initial_place: Point<i32, Logical> = layer_surface.location();
                    initial_place.x -= output_rect.loc.x;

                    if let Some(wl_surface) = layer_surface.surface().get_surface() {
                        // this surface is a root of a subsurface tree that needs to be drawn
                        if let Err(err) =
                            draw_surface_tree(frame, wl_surface, initial_place, output_scale)
                        {
                            result = Err(err);
                        }

                        // TODO
                        // window_map.popups().with_child_popups(
                        //     &wl_surface,
                        //     initial_place,
                        //     |popup, initial_place| {
                        //         let location = popup.popup.location();
                        //         let draw_location = *initial_place + location;
                        //         if let Some(wl_surface) = popup.popup.get_surface() {
                        //             if let Err(err) = draw_surface_tree(
                        //                 renderer,
                        //                 frame,
                        //                 &wl_surface,
                        //                 draw_location,
                        //                 output_scale,
                        //                 log,
                        //             ) {
                        //                 result = Err(err);
                        //             }
                        //         }
                        //         *initial_place = draw_location;
                        //     },
                        // );
                    }
                    //
                });
        }

        result
    }
}

pub fn draw_dnd_icon(
    frame: &mut RenderFrame,
    surface: &wl_surface::WlSurface,
    location: Point<i32, Logical>,
    output_scale: f64,
) -> Result<(), SwapBuffersError> {
    if get_role(surface) != Some("dnd_icon") {
        warn!("Trying to display as a dnd icon a surface that does not have the DndIcon role.");
    }
    draw_surface_tree(frame, surface, location, output_scale)
}
