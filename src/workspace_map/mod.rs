use generational_arena::{Arena, Index};
use smithay::utils::{Logical, Point};

use crate::{output_manager::Output, utils::AsWlSurface, workspace::Workspace};

#[derive(Default, Debug)]
pub struct WorkspaceMap {
    workspaces: Arena<Workspace>,
    outputs: Vec<Output>,
    active_workspace: Option<Index>,
}

impl WorkspaceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn refresh(&mut self) {
        for (_, space) in self.workspaces.iter_mut() {
            space.refresh();
        }
    }

    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }

    pub fn map_output(&mut self, mut output: Output) {
        let mut x = 0;

        for output in self.outputs.iter() {
            let id = output.active_workspace();
            let space = self.workspaces.get(id).unwrap();
            x += space.output_geometry(output).unwrap().size.w;
        }

        let mut space = Workspace::new();
        space.map_output(&output, 1.0, (x, 0));

        let index = self.workspaces.insert(space);

        if self.active_workspace.is_none() {
            self.active_workspace = Some(index);
        }

        output.set_active_workspace(index);

        self.outputs.push(output);
    }

    pub fn output_under(&mut self, point: Point<f64, Logical>) -> Option<&Output> {
        for output in self.outputs.iter() {
            let id = output.active_workspace();
            let workspaces = self.workspaces.get(id).unwrap();
            let geo = workspaces.output_geometry(output).unwrap();

            if geo.to_f64().contains(point) {
                return Some(output);
            }
        }

        None
    }

    pub fn update_mouse_pos(&mut self, point: Point<f64, Logical>) {
        for output in self.outputs.iter() {
            let id = output.active_workspace();
            let workspaces = self.workspaces.get(id).unwrap();
            let geo = workspaces.output_geometry(output).unwrap();

            if geo.to_f64().contains(point) {
                self.active_workspace = Some(id);
                break;
            }
        }
    }

    pub fn active_workspace(&self) -> &Workspace {
        let id = self
            .active_workspace
            .unwrap_or_else(|| self.outputs.first().unwrap().active_workspace());
        self.workspaces.get(id).unwrap()
    }

    pub fn active_workspace_mut(&mut self) -> &mut Workspace {
        let id = self
            .active_workspace
            .unwrap_or_else(|| self.outputs.first().unwrap().active_workspace());
        self.workspaces.get_mut(id).unwrap()
    }

    pub fn visible_workspace_for_output(&self, output: &Output) -> &Workspace {
        let id = output.active_workspace();
        self.workspaces.get(id).unwrap()
    }

    pub fn visible_workspace_for_output_mut(&mut self, output: &Output) -> &mut Workspace {
        let id = output.active_workspace();
        self.workspaces.get_mut(id).unwrap()
    }

    pub fn workspace_for_surface<S: AsWlSurface>(&self, surface: &S) -> Option<&Workspace> {
        let wl_surface = surface.as_surface().unwrap();

        self.workspaces
            .iter()
            .find(|(_, space)| space.window_for_surface(wl_surface).is_some())
            .map(|(_, w)| w)
    }

    pub fn workspace_for_surface_mut<S: AsWlSurface>(&mut self, surface: &S) -> &mut Workspace {
        let wl_surface = surface.as_surface().unwrap();

        self.workspaces
            .iter_mut()
            .find(|(_, space)| space.window_for_surface(wl_surface).is_some())
            .unwrap()
            .1
    }

    pub fn workspace_mut(&mut self, id: Index) -> &mut Workspace {
        self.workspaces.get_mut(id).unwrap()
    }

    pub fn visible_workspaces(&self) -> Vec<Index> {
        self.outputs.iter().map(|o| o.active_workspace()).collect()
    }
}
