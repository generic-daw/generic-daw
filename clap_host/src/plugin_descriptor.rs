use clack_host::plugin;
use std::{
	ffi::CStr,
	fmt::{Display, Formatter},
	str,
	sync::Arc,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PluginDescriptor {
	pub name: Arc<str>,
	pub id: Arc<CStr>,
}

impl Display for PluginDescriptor {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.name.fmt(f)
	}
}

impl TryFrom<&plugin::PluginDescriptor> for PluginDescriptor {
	type Error = Option<str::Utf8Error>;

	fn try_from(value: &plugin::PluginDescriptor) -> Result<Self, Self::Error> {
		Ok(Self {
			name: value.name().ok_or(None)?.to_str()?.into(),
			id: value.id().ok_or(None)?.into(),
		})
	}
}
