use crate::clap_host::ClapPlugin;
use std::rc::Rc;

/// SAFETY: ONLY USE THIS TO GET THE PLUGIN
/// FROM `iced::window::run_with_handle` TO
/// THE `clap_host` VIEW
#[derive(Clone, Debug)]
pub struct ClapPluginWrapper {
    pub inner: Rc<ClapPlugin>,
}

#[expect(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for ClapPluginWrapper {}
unsafe impl Sync for ClapPluginWrapper {}

impl ClapPluginWrapper {
    pub fn new(inner: ClapPlugin) -> Self {
        Self {
            inner: Rc::new(inner),
        }
    }

    pub fn into_inner(self) -> ClapPlugin {
        Rc::into_inner(self.inner).unwrap()
    }
}
