use std::cell::{Cell, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use anodium_protocol::server::{AnodiumProtocol, AnodiumProtocolOutput};
use calloop::channel::Sender;
use smithay::desktop;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::utils::{Logical, Rectangle, Size};
use smithay::wayland::output::Output as SmithayOutput;

use smithay::wayland::output::{Mode, PhysicalProperties};
use smithay::wayland::seat::ModifiersState;

use smithay_egui::{EguiFrame, EguiMode, EguiState};

use crate::config::eventloop::ConfigEvent;
use crate::config::outputs::shell::Shell;
use crate::workspace::Workspace;

/// Inmutable description of phisical output
/// Used before wayland output is created
#[derive(Debug)]
pub struct OutputDescriptor {
    pub name: String,
    pub physical_properties: PhysicalProperties,
}

impl Clone for OutputDescriptor {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            // TODO: Add PhysicalProperties::Clone to smithay
            physical_properties: PhysicalProperties {
                size: self.physical_properties.size,
                subpixel: self.physical_properties.subpixel,
                make: self.physical_properties.make.clone(),
                model: self.physical_properties.model.clone(),
            },
        }
    }
}

struct Data {
    _anodium_protocol_output: AnodiumProtocolOutput,

    possible_modes: Vec<Mode>,

    egui: RefCell<EguiState>,
    egui_shell: Shell,

    #[cfg(feature = "debug")]
    fps_ticker: fps_ticker::Fps,

    config_tx: Sender<ConfigEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    output: SmithayOutput,
}

impl Output {
    pub fn wrap(output: SmithayOutput) -> Self {
        Self { output }
    }

    pub fn from_resource(output: &WlOutput) -> Option<Self> {
        SmithayOutput::from_resource(output).map(Self::wrap)
    }
}

impl Output {
    pub fn new(
        output: SmithayOutput,
        anodium_protocol: &mut AnodiumProtocol,
        possible_modes: Vec<Mode>,
        config_tx: Sender<ConfigEvent>,
    ) -> Self {
        let mut anodium_protocol_output = anodium_protocol.new_output();
        anodium_protocol_output.set_name(output.name());

        let mut egui = EguiState::new(EguiMode::Reactive);
        egui.set_zindex(0);
        let mut visuals = egui::style::Visuals {
            window_corner_radius: 0.0,
            ..Default::default()
        };

        visuals.widgets.inactive.corner_radius = 0.0;
        visuals.widgets.noninteractive.corner_radius = 0.0;
        visuals.widgets.hovered.corner_radius = 0.0;
        visuals.widgets.active.corner_radius = 0.0;
        visuals.widgets.open.corner_radius = 0.0;
        visuals.window_shadow.extrusion = 0.0;

        egui.context().set_visuals(visuals);

        let mut default_workspace = Workspace::new();
        let insert_workspaces = Rc::new(RefCell::new(HashMap::new()));

        let added = output.user_data().insert_if_missing(move || Data {
            _anodium_protocol_output: anodium_protocol_output,

            possible_modes,
            egui: RefCell::new(egui),
            egui_shell: Shell::new(),
            #[cfg(feature = "debug")]
            fps_ticker: fps_ticker::Fps::default(),

            config_tx,
        });
        assert!(added);

        default_workspace.map_output(&output, 1.0, (0, 0));
        insert_workspaces
            .borrow_mut()
            .insert("0".to_owned(), default_workspace);

        Self { output }
    }

    fn data(&self) -> &Data {
        self.output.user_data().get().unwrap()
    }

    pub fn possible_modes(&self) -> &[Mode] {
        &self.data().possible_modes
    }

    pub fn layer_map(&self) -> RefMut<desktop::LayerMap> {
        desktop::layer_map_for_output(&self.output)
    }

    pub fn config_tx(&self) -> &Sender<ConfigEvent> {
        &self.data().config_tx
    }

    pub fn mode(&mut self, mode: Mode) {
        self.output
            .change_current_state(Some(mode), None, None, None)
    }
}

impl Output {
    pub fn egui(&self) -> RefMut<EguiState> {
        self.data().egui.borrow_mut()
    }

    pub fn egui_shell(&self) -> &Shell {
        &self.data().egui_shell
    }

    pub fn render_egui_shell(
        &self,
        start_time: &Instant,
        modifiers: &ModifiersState,
        config_tx: &Sender<ConfigEvent>,
    ) -> EguiFrame {
        let scale = self.output.current_scale();
        let size = self.output.current_mode().unwrap().size;

        let data = self.data();
        data.egui.borrow_mut().run(
            |ctx| {
                //TODO - fix that in smithay, currently if crashes if egui does not have any element
                egui::Area::new("main")
                    .anchor(egui::Align2::LEFT_TOP, (10.0, 10.0))
                    .show(ctx, |_ui| {});
                data.egui_shell.render(ctx, config_tx);
            },
            Rectangle::from_loc_and_size((0, 0), size.to_logical(scale)),
            scale as f64,
            1.0,
            start_time,
            *modifiers,
        )
    }

    #[cfg(feature = "debug")]
    pub fn tick_fps(&self) {
        self.data().fps_ticker.tick();
    }

    #[cfg(feature = "debug")]
    pub fn get_fps(&self) -> u32 {
        self.data().fps_ticker.avg().round() as u32
    }
}

impl std::ops::Deref for Output {
    type Target = SmithayOutput;

    fn deref(&self) -> &Self::Target {
        &self.output
    }
}

impl std::ops::DerefMut for Output {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.output
    }
}
