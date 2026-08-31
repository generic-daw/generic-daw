use crate::Feature;
use clack_host::plugin;
use std::{
	ffi::CStr,
	fmt::{Display, Formatter},
	path::Path,
	str,
	sync::Arc,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PluginDescriptor {
	pub id: Arc<CStr>,
	pub name: Arc<str>,
	pub vendor: Option<Arc<str>>,
	pub version: Option<Arc<str>>,
	pub features: Arc<[Feature]>,
	pub path: Arc<Path>,
}

impl PluginDescriptor {
	pub fn try_new(
		value: &plugin::PluginDescriptor,
		path: &Arc<Path>,
	) -> Result<Self, Option<str::Utf8Error>> {
		Ok(Self {
			id: value.id().ok_or(None)?.into(),
			name: value.name().ok_or(None)?.to_str()?.into(),
			vendor: value.vendor().map(CStr::to_str).transpose()?.map(Arc::from),
			version: value
				.version()
				.map(CStr::to_str)
				.transpose()?
				.map(Arc::from),
			features: value.features().filter_map(Feature::parse).collect(),
			path: path.clone(),
		})
	}
}

impl Display for PluginDescriptor {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.name.fmt(f)
	}
}
