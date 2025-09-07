use clack_extensions::params::{ParamInfo, ParamInfoBuffer, ParamInfoFlags, PluginParams};
use clack_host::{prelude::*, utils::Cookie};
use std::{mem::MaybeUninit, ops::RangeInclusive, sync::Arc};

#[derive(Debug)]
pub struct Param {
	pub id: ClapId,
	pub flags: ParamInfoFlags,
	pub cookie: Cookie,
	pub name: Arc<str>,
	pub range: RangeInclusive<f32>,
	pub reset: f32,
	pub value: f32,
	pub value_text: Option<Arc<str>>,
}

impl TryFrom<ParamInfo<'_>> for Param {
	type Error = ();

	fn try_from(value: ParamInfo<'_>) -> Result<Self, Self::Error> {
		Ok(Self {
			id: value.id,
			flags: value.flags,
			cookie: value.cookie,
			name: str::from_utf8(value.name).map_err(|_| ())?.into(),
			range: value.min_value as f32..=value.max_value as f32,
			reset: value.default_value as f32,
			value: value.default_value as f32,
			value_text: None,
		})
	}
}

impl Param {
	pub fn all(plugin: &mut PluginMainThreadHandle<'_>) -> Option<Box<[Self]>> {
		let ext = plugin.get_extension::<PluginParams>()?;

		let count = ext.count(plugin) as usize;
		let buffer = &mut ParamInfoBuffer::new();

		let mut params = (0..)
			.filter_map(|index| ext.get_info(plugin, index, buffer).map(Self::try_from))
			.take(count)
			.flatten()
			.collect::<Box<_>>();

		for param in &mut params {
			param.rescan_value(plugin);
		}

		Some(params)
	}

	pub fn rescan_value(&mut self, plugin: &mut PluginMainThreadHandle<'_>) {
		if let Some(ext) = plugin.get_extension::<PluginParams>()
			&& let Some(value) = ext.get_value(plugin, self.id)
		{
			self.update_with_value(value, plugin);
		}
	}

	pub fn update_with_value(&mut self, value: f64, plugin: &mut PluginMainThreadHandle<'_>) {
		self.value = value as f32;

		let mut buf = [MaybeUninit::uninit(); 32];

		self.value_text = if let Some(ext) = plugin.get_extension::<PluginParams>()
			&& let Ok(value_text) = ext.value_to_text(plugin, self.id, value, &mut buf)
			&& let Ok(value_text) = str::from_utf8(value_text)
			&& !value_text.is_empty()
		{
			Some(value_text.into())
		} else {
			None
		};
	}
}
