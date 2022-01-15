use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use std::time::Instant;

use calloop::channel::Sender;

use smithay_egui::{EguiFrame, EguiState};

use smithay::backend::renderer::gles2::{Gles2Renderer, Gles2Texture};
use smithay::{
    reexports::wayland_server::{protocol::wl_output::WlOutput, Display, Global, UserDataMap},
    utils::{Logical, Point, Rectangle, Size},
    wayland::{
        output::{self, Mode, PhysicalProperties},
        seat::ModifiersState,
    },
};

use derivative::Derivative;

use image::{self, DynamicImage};

use crate::config::eventloop::ConfigEvent;
use crate::config::outputs::shell::Shell;
use crate::render::renderer::import_bitmap;

use super::layer_map::LayerMap;

#[derive(Derivative)]
#[derivative(Debug)]
struct Inner {
    name: String,
    output: output::Output,
    global: Option<Global<WlOutput>>,
    current_mode: Mode,
    pending_mode_change: bool,
    possible_modes: Vec<Mode>,
    scale: f64,
    location: Point<i32, Logical>,

    active_workspace: String,
    userdata: UserDataMap,

    layer_map: LayerMap,

    wallpaper: Option<DynamicImage>,
    wallpaper_texture: Option<Gles2Texture>,

    #[derivative(Debug = "ignore")]
    egui: Option<EguiState>,
    shell: Shell,

    fps: f64,
}

impl Inner {
    pub fn possible_modes(&self) -> Vec<Mode> {
        self.possible_modes.clone()
    }

    pub fn update_mode(&mut self, mode: Mode) {
        let scale = self.scale.round() as i32;

        self.output.delete_mode(self.current_mode);
        self.output
            .change_current_state(Some(mode), None, Some(scale), None);
        self.output.set_preferred(mode);

        self.current_mode = mode;
        self.pending_mode_change = true;
    }

    pub fn update_scale(&mut self, scale: f64) {
        if self.scale.round() as u32 != scale.round() as u32 {
            let current_mode = self.current_mode;

            self.scale = scale;

            self.output.change_current_state(
                Some(current_mode),
                None,
                Some(scale.round() as i32),
                None,
            );
        }
    }

    pub fn set_wallpaper(&mut self, path: &str) {
        let image = image::open(path).unwrap();
        self.wallpaper = Some(image);
        self.wallpaper_texture = None;
    }

    pub fn get_wallpaper(&mut self, renderer: &mut Gles2Renderer) -> Option<Gles2Texture> {
        if let Some(wallpaper_texture) = &self.wallpaper_texture {
            Some(wallpaper_texture.clone())
        } else if let Some(wallpaper) = &self.wallpaper {
            if let Ok(wallpaper_texture) = import_bitmap(
                renderer,
                &wallpaper.to_rgba8(),
                Some(self.current_mode.size.into()),
            ) {
                self.wallpaper_texture = Some(wallpaper_texture.clone());
                self.wallpaper = None;
                Some(wallpaper_texture)
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct Output {
    inner: Rc<RefCell<Inner>>,
}

impl Output {
    pub fn new<N>(
        name: N,
        location: Point<i32, Logical>,
        display: &mut Display,
        physical: PhysicalProperties,
        mode: Mode,
        possible_modes: Vec<Mode>,
        active_workspace: String,
        logger: slog::Logger,
    ) -> Self
    where
        N: AsRef<str>,
    {
        let egui = EguiState::new();
        let mut visuals = egui::style::Visuals::default();
        visuals.window_corner_radius = 0.0;

        egui.context().set_visuals(visuals);

        let (output, global) = output::Output::new(display, name.as_ref().into(), physical, logger);

        let scale = 1.0f64;

        output.change_current_state(Some(mode), None, Some(scale.round() as i32), Some(location));
        output.set_preferred(mode);

        Self {
            inner: Rc::new(RefCell::new(Inner {
                name: name.as_ref().to_owned(),
                global: Some(global),
                output,
                location,
                pending_mode_change: false,
                current_mode: mode,
                possible_modes,
                scale,

                active_workspace,
                userdata: Default::default(),

                layer_map: Default::default(),
                wallpaper: None,
                wallpaper_texture: None,

                egui: Some(egui),
                shell: Shell::new(),
                fps: 0.0,
            })),
        }
    }

    pub fn active_workspace(&self) -> String {
        self.inner.borrow().active_workspace.clone()
    }
    pub fn set_active_workspace(&mut self, key: String) {
        self.inner.borrow_mut().active_workspace = key;
    }

    pub fn location(&self) -> Point<i32, Logical> {
        self.inner.borrow().location
    }
    pub fn set_location(&mut self, location: Point<i32, Logical>) {
        self.inner.borrow_mut().location = location;
        self.inner
            .borrow()
            .output
            .change_current_state(None, None, None, Some(location));
    }

    pub fn userdata(&self) -> Ref<UserDataMap> {
        Ref::map(self.inner.borrow(), |b| &b.userdata)
    }

    pub fn geometry(&self) -> Rectangle<i32, Logical> {
        let loc = self.location();
        let size = self.size();

        Rectangle { loc, size }
    }

    pub fn usable_geometry(&self) -> Rectangle<i32, Logical> {
        let mut ret = self.geometry();

        let inner = self.inner.borrow();

        ret.loc.x += inner.layer_map.exclusive_zone().left as i32;
        ret.size.w -= inner.layer_map.exclusive_zone().left as i32;

        ret.loc.y += inner.layer_map.exclusive_zone().top as i32;
        ret.size.h -= inner.layer_map.exclusive_zone().top as i32;

        ret.size.w -= inner.layer_map.exclusive_zone().left as i32;
        ret.size.h -= inner.layer_map.exclusive_zone().bottom as i32;

        ret
    }

    pub fn size(&self) -> Size<i32, Logical> {
        let inner = self.inner.borrow();

        inner
            .current_mode
            .size
            .to_f64()
            .to_logical(inner.scale)
            .to_i32_round()
    }

    pub fn scale(&self) -> f64 {
        self.inner.borrow().scale
    }

    pub fn name(&self) -> String {
        self.inner.borrow().name.clone()
    }

    #[allow(unused)]
    pub fn current_mode(&self) -> Mode {
        self.inner.borrow().current_mode
    }

    pub fn layer_map(&self) -> Ref<LayerMap> {
        Ref::map(self.inner.borrow(), |b| &b.layer_map)
    }

    pub fn layer_map_mut(&mut self) -> RefMut<LayerMap> {
        RefMut::map(self.inner.borrow_mut(), |b| &mut b.layer_map)
    }

    pub fn inner_output(&self) -> Ref<output::Output> {
        Ref::map(self.inner.borrow(), |b| &b.output)
    }

    pub fn update_mode(&mut self, mode: Mode) {
        self.inner.borrow_mut().update_mode(mode);
    }

    #[allow(unused)]
    pub fn possible_modes(&self) -> Vec<Mode> {
        self.inner.borrow().possible_modes()
    }

    pub fn update_scale(&mut self, scale: f64) {
        self.inner.borrow_mut().update_scale(scale);
    }

    pub fn set_wallpaper(&mut self, path: &str) {
        self.inner.borrow_mut().set_wallpaper(path);
    }

    pub fn get_wallpaper(&self, renderer: &mut Gles2Renderer) -> Option<Gles2Texture> {
        self.inner.borrow_mut().get_wallpaper(renderer)
    }

    pub fn render_shell(
        &self,
        start_time: &Instant,
        modifiers: &ModifiersState,
        config_tx: &Sender<ConfigEvent>,
    ) -> EguiFrame {
        let size = self.size();
        let scale = self.scale();
        let mut egui;
        {
            let mut inner = self.inner.borrow_mut();
            egui = inner.egui.take().unwrap();
        }
        let frame;
        {
            let inner = self.inner.borrow();
            frame = egui.run(
                |ctx| {
                    inner.shell.render(ctx, config_tx);
                },
                // Just render it over the whole window, but you may limit the area
                Rectangle::from_loc_and_size((0, 0), size),
                size.to_physical(1),
                scale,
                scale as f32,
                start_time,
                modifiers.clone(),
            );
        }
        let mut inner = self.inner.borrow_mut();
        inner.egui = Some(egui);
        frame
    }

    pub fn pending_mode_change(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.pending_mode_change {
            inner.pending_mode_change = false;
            true
        } else {
            false
        }
    }

    pub fn shell(&self) -> Shell {
        self.inner.borrow().shell.clone()
    }

    pub fn get_fps(&self) -> f64 {
        self.inner.borrow().fps
    }

    pub fn update_fps(&self, fps: f64) {
        self.inner.borrow_mut().fps = fps;
    }

    pub fn egui(&self) -> RefMut<EguiState> {
        RefMut::map(self.inner.borrow_mut(), |o| o.egui.as_mut().unwrap())
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.global.take().unwrap().destroy();
    }
}

impl PartialEq for Output {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}
