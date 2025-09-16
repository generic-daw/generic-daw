use crate::host::Host;
use clack_extensions::params::{ParamInfo, ParamInfoBuffer, ParamInfoFlags};
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
	pub fn all(plugin: &mut PluginInstance<Host>) -> Option<Box<[Self]>> {
		let ext = *plugin.access_shared_handler(|s| s.ext.params.get())?;

		let count = ext.count(&mut plugin.plugin_handle()) as usize;
		let buffer = &mut ParamInfoBuffer::new();

		let mut params = (0..)
			.filter_map(|index| {
				ext.get_info(&mut plugin.plugin_handle(), index, buffer)
					.map(Self::try_from)
			})
			.take(count)
			.flatten()
			.collect::<Box<_>>();

		for param in &mut params {
			param.rescan_value(plugin);
		}

		Some(params)
	}

	pub fn rescan_value(&mut self, plugin: &mut PluginInstance<Host>) {
		if let Some(&ext) = plugin.access_shared_handler(|s| s.ext.params.get())
			&& let Some(value) = ext.get_value(&mut plugin.plugin_handle(), self.id)
		{
			self.update_with_value(value as f32, plugin);
		}
	}

	pub fn update_with_value(&mut self, value: f32, plugin: &mut PluginInstance<Host>) {
		self.value = value;

		let value = value.into();
		self.value_text = if let Some(&ext) = plugin.access_shared_handler(|s| s.ext.params.get())
			&& let Ok(value_text) = ext.value_to_text(
				&mut plugin.plugin_handle(),
				self.id,
				value,
				&mut [MaybeUninit::zeroed(); 32],
			) && let Ok(value_text) = str::from_utf8(value_text)
			&& !value_text.is_empty()
		{
			Some(value_text.into())
		} else {
			None
		};
	}
}
