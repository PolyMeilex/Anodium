use std::cell::{Ref, RefCell, RefMut};
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::ops::Index;
use std::rc::Rc;

use derivative::Derivative;

use indexmap::Equivalent;
use smithay::backend::input::Device;
use smithay::desktop;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct WorkspaceInner {
    #[derivative(Debug = "ignore")]
    space: desktop::Space,
    name: String,
}

#[derive(Clone, Debug)]
pub struct Workspace {
    name: String,
    inner: Rc<RefCell<WorkspaceInner>>,
}

impl Workspace {
    pub fn new(name: String) -> Self {
        Self {
            name: name.clone(),
            inner: Rc::new(RefCell::new(WorkspaceInner {
                space: desktop::Space::new(slog_scope::logger()),
                name,
            })),
        }
    }

    pub fn space(&self) -> Ref<'_, desktop::Space> {
        Ref::map(self.inner.borrow(), |f| &f.space)
    }

    pub fn space_mut(&self) -> RefMut<'_, desktop::Space> {
        RefMut::map(self.inner.borrow_mut(), |f| &mut f.space)
    }
}

impl PartialEq for Workspace {
    fn eq(&self, other: &Self) -> bool {
        self.inner.borrow().name == other.inner.borrow().name
    }
}
impl Eq for Workspace {}

impl Hash for Workspace {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.borrow().name.hash(state);
    }
}

impl core::borrow::Borrow<str> for Workspace {
    fn borrow(&self) -> &str {
        &self.name
    }
}
