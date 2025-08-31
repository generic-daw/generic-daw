use clack_extensions::params::{ParamInfo, ParamInfoBuffer, ParamInfoFlags, PluginParams};
use clack_host::{
	plugin::PluginMainThreadHandle,
	utils::{ClapId, Cookie},
};
use std::{ops::RangeInclusive, sync::Arc};

#[derive(Debug)]
pub struct Param {
	pub id: ClapId,
	pub flags: ParamInfoFlags,
	pub cookie: Cookie,
	pub name: Arc<str>,
	pub range: RangeInclusive<f32>,
	pub reset: f32,
	pub value: f32,
}

impl TryFrom<ParamInfo<'_>> for Param {
	type Error = ();

	fn try_from(value: ParamInfo<'_>) -> Result<Self, Self::Error> {
		Ok(Self {
			id: value.id,
			flags: value.flags,
			cookie: value.cookie,
			name: String::from_utf8(value.name.to_owned())
				.map_err(|_| ())?
				.into(),
			range: value.min_value as f32..=value.max_value as f32,
			reset: value.default_value as f32,
			value: value.default_value as f32,
		})
	}
}

impl Param {
	pub fn all(plugin: &mut PluginMainThreadHandle<'_>, ext: PluginParams) -> Box<[Self]> {
		let count = ext.count(plugin) as usize;
		let buffer = &mut ParamInfoBuffer::new();

		(0..)
			.filter_map(|index| ext.get_info(plugin, index, buffer).map(Self::try_from))
			.take(count)
			.flatten()
			.collect()
	}

	pub fn rescan_value(&mut self, plugin: &mut PluginMainThreadHandle<'_>, ext: PluginParams) {
		let Some(value) = ext.get_value(plugin, self.id) else {
			return;
		};

		self.value = value as f32;
	}
}
