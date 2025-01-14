use crate::ClapPluginGui;
use std::thread::ThreadId;

#[derive(Debug)]
pub struct ClapPluginGuiWrapper {
    inner: Option<ClapPluginGui>,
    id: ThreadId,
}

#[expect(clippy::non_send_fields_in_send_ty)]
// SAFETY:
// this is constructed and dropped on the main thread, because it only goes to a different thread within iced's async runtime, where it is never dropped
unsafe impl Send for ClapPluginGuiWrapper {}

impl Drop for ClapPluginGuiWrapper {
    fn drop(&mut self) {
        if self.inner.is_some() {
            let id = std::thread::current().id();
            assert_eq!(self.id, id);
        }
    }
}

impl ClapPluginGuiWrapper {
    #[must_use]
    pub fn new(inner: ClapPluginGui) -> Self {
        let id = std::thread::current().id();

        Self {
            inner: Some(inner),
            id,
        }
    }

    #[must_use]
    pub fn inner(&self) -> &ClapPluginGui {
        let id = std::thread::current().id();
        assert_eq!(self.id, id);

        self.inner.as_ref().unwrap()
    }

    #[must_use]
    pub fn into_inner(mut self) -> ClapPluginGui {
        let id = std::thread::current().id();
        assert_eq!(self.id, id);

        let inner = self.inner.take().unwrap();
        drop(self);

        inner
    }
}
