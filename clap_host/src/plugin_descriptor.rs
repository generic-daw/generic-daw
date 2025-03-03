use crate::PluginType;
use clack_host::factory;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PluginDescriptor {
    pub name: Box<str>,
    pub id: Box<str>,
    pub ty: PluginType,
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
            ty: value.features().find_map(|f| f.try_into().ok()).unwrap(),
        })
    }
}

impl PartialEq<PluginDescriptor> for factory::PluginDescriptor<'_> {
    fn eq(&self, other: &PluginDescriptor) -> bool {
        self.name().and_then(|name| name.to_str().ok()) == Some(&other.name)
            && self.id().and_then(|id| id.to_str().ok()) == Some(&other.id)
    }
}
