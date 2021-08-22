#![allow(clippy::too_many_arguments)]

use std::{cell::RefCell, sync::Mutex};

#[cfg(feature = "image")]
use image::{ImageBuffer, Rgba};
use slog::Logger;
#[cfg(feature = "image")]
use smithay::backend::renderer::gles2::{Gles2Error, Gles2Renderer, Gles2Texture};
use smithay::{
    backend::{
        renderer::{buffer_type, BufferType, Frame, ImportAll, Transform},
        SwapBuffersError,
    },
    reexports::wayland_server::protocol::{wl_buffer, wl_surface},
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::{
            get_role, with_states, with_surface_tree_upward, Damage, SubsurfaceCachedState,
            SurfaceAttributes, TraversalAction,
        },
        seat::CursorImageAttributes,
    },
};

use crate::{desktop_layout::Window, render::renderer::RenderFrame, shell::SurfaceData, state::MainState};

struct BufferTextures<T> {
    buffer: Option<wl_buffer::WlBuffer>,
    texture: T,
}

impl<T> Drop for BufferTextures<T> {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            buffer.release();
        }
    }
}

pub fn draw_cursor(
    frame: &mut RenderFrame,
    surface: &wl_surface::WlSurface,
    location: Point<i32, Logical>,
    output_scale: f64,
    log: &Logger,
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
                log,
                "Trying to display as a cursor a surface that does not have the CursorImage role."
            );
            (0, 0).into()
        }
    };
    draw_surface_tree(frame, surface, location - delta, output_scale, log)
}

fn draw_surface_tree(
    frame: &mut RenderFrame,
    root: &wl_surface::WlSurface,
    location: Point<i32, Logical>,
    output_scale: f64,
    log: &Logger,
) -> Result<(), SwapBuffersError> {
    let mut result = Ok(());

    let renderer = &mut frame.context.renderer;
    let frame = &mut frame.frame;
    with_surface_tree_upward(
        root,
        location,
        |_surface, states, location| {
            let mut location = *location;
            // Pull a new buffer if available
            if let Some(data) = states.data_map.get::<RefCell<SurfaceData>>() {
                let mut data = data.borrow_mut();
                let attributes = states.cached_state.current::<SurfaceAttributes>();
                if data.texture.is_none() {
                    if let Some(buffer) = data.buffer.take() {
                        let damage = attributes
                            .damage
                            .iter()
                            .map(|dmg| match dmg {
                                Damage::Buffer(rect) => *rect,
                                // TODO also apply transformations
                                Damage::Surface(rect) => rect.to_buffer(attributes.buffer_scale),
                            })
                            .collect::<Vec<_>>();

                        match renderer.import_buffer(&buffer, Some(states), &damage) {
                            Some(Ok(m)) => {
                                let texture_buffer = if let Some(BufferType::Shm) = buffer_type(&buffer) {
                                    buffer.release();
                                    None
                                } else {
                                    Some(buffer)
                                };
                                data.texture = Some(Box::new(BufferTextures {
                                    buffer: texture_buffer,
                                    texture: m,
                                }))
                            }
                            Some(Err(err)) => {
                                warn!(log, "Error loading buffer: {:?}", err);
                                buffer.release();
                            }
                            None => {
                                error!(log, "Unknown buffer format for: {:?}", buffer);
                                buffer.release();
                            }
                        }
                    }
                }
                // Now, should we be drawn ?
                if data.texture.is_some() {
                    // if yes, also process the children
                    if states.role == Some("subsurface") {
                        let current = states.cached_state.current::<SubsurfaceCachedState>();
                        location += current.location;
                    }
                    TraversalAction::DoChildren(location)
                } else {
                    // we are not displayed, so our children are neither
                    TraversalAction::SkipChildren
                }
            } else {
                // we are not displayed, so our children are neither
                TraversalAction::SkipChildren
            }
        },
        |_surface, states, location| {
            let mut location = *location;
            if let Some(ref data) = states.data_map.get::<RefCell<SurfaceData>>() {
                let mut data = data.borrow_mut();
                let buffer_scale = data.buffer_scale;
                if let Some(texture) = data
                    .texture
                    .as_mut()
                    .and_then(|x| x.downcast_mut::<BufferTextures<Gles2Texture>>())
                {
                    // we need to re-extract the subsurface offset, as the previous closure
                    // only passes it to our children
                    if states.role == Some("subsurface") {
                        let current = states.cached_state.current::<SubsurfaceCachedState>();
                        location += current.location;
                    }
                    if let Err(err) = frame.render_texture_at(
                        &texture.texture,
                        location.to_f64().to_physical(output_scale as f64).to_i32_round(),
                        buffer_scale,
                        output_scale as f64,
                        Transform::Normal, /* TODO */
                        1.0,
                    ) {
                        result = Err(err.into());
                    }
                }
            }
        },
        |_, _, _| true,
    );

    result
}

impl MainState {
    pub fn draw_windows(
        &self,
        frame: &mut RenderFrame,
        output_rect: Rectangle<i32, Logical>,
        output_scale: f64,
        log: &::slog::Logger,
    ) -> Result<(), SwapBuffersError> {
        let mut render = move |window: &Window| {
            let mut initial_place = window.render_location();

            // skip windows that do not overlap with a given output
            if !output_rect.overlaps(window.bbox()) {
                return;
            }
            initial_place.x -= output_rect.loc.x;

            if let Some(wl_surface) = window.surface().as_ref() {
                // this surface is a root of a subsurface tree that needs to be drawn
                if let Err(err) = draw_surface_tree(frame, &wl_surface, initial_place, output_scale, log) {
                    error!(log, "{:?}", err);
                }
                // furthermore, draw its popups
                // let toplevel_geometry_offset = window.geometry().loc;

                // TODO
                // self.window_map.borrow().popups().with_child_popups(
                //     &wl_surface,
                //     initial_place + toplevel_geometry_offset,
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
        };

        // redraw the frame, in a simple but inneficient way
        for workspace in self.desktop_layout.visible_workspaces() {
            for window in workspace.windows().iter().rev() {
                render(window);
            }
        }

        if let Some(state) = self.desktop_layout.grabed_window.borrow().as_ref() {
            render(&state.window);
        }

        Ok(())
    }
}

// pub fn draw_layers<R, E, F, T>(
//     renderer: &mut R,
//     frame: &mut F,
//     window_map: &WindowMap,
//     layer: Layer,
//     output_rect: Rectangle<i32, Logical>,
//     output_scale: f32,
//     log: &::slog::Logger,
// ) -> Result<(), SwapBuffersError>
// where
//     R: Renderer<Error = E, TextureId = T, Frame = F> + ImportAll,
//     F: Frame<Error = E, TextureId = T>,
//     E: std::error::Error + Into<SwapBuffersError>,
//     T: Texture + 'static,
// {
//     let mut result = Ok(());

//     window_map
//         .layers
//         .with_layers_from_bottom_to_top(&layer, |layer_surface| {
//             // skip layers that do not overlap with a given output
//             if !output_rect.overlaps(layer_surface.bbox) {
//                 return;
//             }

//             let mut initial_place: Point<i32, Logical> = layer_surface.location;
//             initial_place.x -= output_rect.loc.x;

//             if let Some(wl_surface) = layer_surface.surface.get_surface() {
//                 // this surface is a root of a subsurface tree that needs to be drawn
//                 if let Err(err) =
//                     draw_surface_tree(renderer, frame, wl_surface, initial_place, output_scale, log)
//                 {
//                     result = Err(err);
//                 }

//                 window_map
//                     .popups()
//                     .with_child_popups(&wl_surface, initial_place, |popup, initial_place| {
//                         let location = popup.popup.location();
//                         let draw_location = *initial_place + location;
//                         if let Some(wl_surface) = popup.popup.get_surface() {
//                             if let Err(err) = draw_surface_tree(
//                                 renderer,
//                                 frame,
//                                 &wl_surface,
//                                 draw_location,
//                                 output_scale,
//                                 log,
//                             ) {
//                                 result = Err(err);
//                             }
//                         }
//                         *initial_place = draw_location;
//                     });
//             }
//         });

//     result
// }

pub fn draw_dnd_icon(
    frame: &mut RenderFrame,
    surface: &wl_surface::WlSurface,
    location: Point<i32, Logical>,
    output_scale: f64,
    log: &::slog::Logger,
) -> Result<(), SwapBuffersError> {
    if get_role(surface) != Some("dnd_icon") {
        warn!(
            log,
            "Trying to display as a dnd icon a surface that does not have the DndIcon role."
        );
    }
    draw_surface_tree(frame, surface, location, output_scale, log)
}

// TODO: Move this to diferent module, this is not wayland specyfic
#[cfg(feature = "debug")]
pub fn draw_fps(frame: &mut RenderFrame, _output_scale: f64, value: u32) -> Result<(), Gles2Error> {
    let ui = &frame.imgui_frame;

    imgui::Window::new(imgui::im_str!("FPS"))
        .size([50.0, 20.0], imgui::Condition::Always)
        .position([0.0, 0.0], imgui::Condition::Always)
        .title_bar(false)
        .build(&ui, || {
            ui.text(&format!("{}FPS", value));
        });

    Ok(())
}

// TODO: Move this to diferent module, this is not wayland specyfic
#[cfg(feature = "image")]
pub fn import_bitmap<C: std::ops::Deref<Target = [u8]>>(
    renderer: &mut Gles2Renderer,
    image: &ImageBuffer<Rgba<u8>, C>,
) -> Result<Gles2Texture, Gles2Error> {
    use smithay::backend::renderer::gles2::ffi;

    renderer.with_context(|renderer, gl| unsafe {
        let mut tex = 0;
        gl.GenTextures(1, &mut tex);
        gl.BindTexture(ffi::TEXTURE_2D, tex);
        gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_WRAP_S, ffi::CLAMP_TO_EDGE as i32);
        gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_WRAP_T, ffi::CLAMP_TO_EDGE as i32);
        gl.TexImage2D(
            ffi::TEXTURE_2D,
            0,
            ffi::RGBA as i32,
            image.width() as i32,
            image.height() as i32,
            0,
            ffi::RGBA,
            ffi::UNSIGNED_BYTE as u32,
            image.as_ptr() as *const _,
        );
        gl.BindTexture(ffi::TEXTURE_2D, 0);

        Gles2Texture::from_raw(
            renderer,
            tex,
            (image.width() as i32, image.height() as i32).into(),
        )
    })
}
