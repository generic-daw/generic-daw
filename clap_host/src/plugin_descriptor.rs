use clack_host::factory;
use std::{
    fmt::{Display, Formatter},
    sync::Arc,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PluginDescriptor {
    pub name: Arc<str>,
    pub id: Arc<str>,
}

impl Display for PluginDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.name.fmt(f)
    }
}

impl TryFrom<factory::PluginDescriptor<'_>> for PluginDescriptor {
    type Error = ();

    fn try_from(value: factory::PluginDescriptor<'_>) -> Result<Self, ()> {
        Ok(Self {
            name: value.name().ok_or(())?.to_str().map_err(|_| ())?.into(),
            id: value.id().ok_or(())?.to_str().map_err(|_| ())?.into(),
        })
    }
}
