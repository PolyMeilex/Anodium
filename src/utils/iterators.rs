use std::collections::HashMap;

use crate::output_map::{Output, OutputMap};
use crate::positioner::Positioner;

pub struct VisibleWorkspaceIter<'a> {
    outputs: std::vec::IntoIter<Output>,
    workspaces: &'a HashMap<String, Box<dyn Positioner>>,
}

impl<'a> VisibleWorkspaceIter<'a> {
    pub fn new(
        outputs: &'a OutputMap,
        workspaces: &'a HashMap<String, Box<dyn Positioner>>,
    ) -> Self {
        Self {
            outputs: outputs.iter(),
            workspaces,
        }
    }
}

impl<'a> Iterator for VisibleWorkspaceIter<'a> {
    type Item = &'a dyn Positioner;

    fn next(&mut self) -> Option<Self::Item> {
        self.outputs.next().map(|output| {
            self.workspaces
                .get(&output.active_workspace())
                .unwrap()
                .as_ref()
        })
    }
}

pub struct VisibleWorkspaceIterMut<'a> {
    keys: std::collections::hash_set::IntoIter<String>,
    workspaces: &'a mut HashMap<String, Box<dyn Positioner>>,
}

impl<'a> VisibleWorkspaceIterMut<'a> {
    pub fn new(
        outputs: &OutputMap,
        workspaces: &'a mut HashMap<String, Box<dyn Positioner>>,
    ) -> Self {
        let mut keys = std::collections::HashSet::new();
        let all_unique = outputs.iter().all(|o| keys.insert(o.active_workspace()));

        if !all_unique {
            slog_scope::error!(
                "One of the outputs don't have unique workspaces asigned to it: {:?}",
                outputs
            );
            slog_scope::warn!("Ignoring duplicate workspace, because of safety requirements");
        }

        Self {
            keys: keys.into_iter(),
            workspaces,
        }
    }
}

impl<'a> Iterator for VisibleWorkspaceIterMut<'a> {
    type Item = &'a mut dyn Positioner;

    fn next(&mut self) -> Option<Self::Item> {
        self.keys.next().map(|key| {
            let workspace = self.workspaces.get_mut(&key).unwrap().as_mut();
            // Unsafe: Self::new checks if every key is unique so we know for sure that every mutable reference will be unique
            // Because of that there will never be 2 muttable refs at the same time
            let relifetimed: &'a mut dyn Positioner = unsafe { std::mem::transmute(workspace) };
            relifetimed
        })
    }
}
