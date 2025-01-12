use crate::clap_host::ClapPlugin;
use std::thread::ThreadId;

#[derive(Debug)]
pub struct ClapPluginWrapper {
    inner: Option<ClapPlugin>,
    id: ThreadId,
}

#[expect(clippy::non_send_fields_in_send_ty)]
// SAFETY:
// this is constructed and dropped on the main thread, and only goes to a different thread within iced's async runtime, where it is never dropped
unsafe impl Send for ClapPluginWrapper {}

// SAFETY:
// this is only ever outside the main thread within iced's async runtime, inside which the `ClapPlugin` is never accessed
unsafe impl Sync for ClapPluginWrapper {}

impl Clone for ClapPluginWrapper {
    fn clone(&self) -> Self {
        unreachable!()
    }
}

impl Drop for ClapPluginWrapper {
    fn drop(&mut self) {
        if self.inner.is_some() {
            let id = std::thread::current().id();
            assert_eq!(self.id, id);
        }
    }
}

impl ClapPluginWrapper {
    pub fn new(inner: ClapPlugin) -> Self {
        let id = std::thread::current().id();

        Self {
            inner: Some(inner),
            id,
        }
    }

    pub fn inner(&self) -> &ClapPlugin {
        let id = std::thread::current().id();
        assert_eq!(self.id, id);

        self.inner.as_ref().unwrap()
    }

    pub fn into_inner(mut self) -> ClapPlugin {
        let id = std::thread::current().id();
        assert_eq!(self.id, id);

        let inner = self.inner.take().unwrap();
        drop(self);

        inner
    }
}
