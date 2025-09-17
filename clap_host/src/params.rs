use crate::host::Host;
use clack_extensions::params::{ParamInfoBuffer, ParamInfoFlags, ParamRescanFlags};
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
	index: u32,
}

impl Param {
	pub fn all(plugin: &mut PluginInstance<Host>) -> Option<Box<[Self]>> {
		let ext = *plugin.access_shared_handler(|s| s.ext.params.get())?;

		let count = ext.count(&mut plugin.plugin_handle());

		let mut params = (0..count)
			.filter_map(|index| Self::try_new(plugin, index))
			.collect::<Box<_>>();

		for param in &mut params {
			param.rescan(plugin, ParamRescanFlags::VALUES);
		}

		Some(params)
	}

	fn try_new(plugin: &mut PluginInstance<Host>, index: u32) -> Option<Self> {
		let ext = *plugin.access_shared_handler(|s| s.ext.params.get())?;
		let mut buffer = ParamInfoBuffer::new();
		let param = ext.get_info(&mut plugin.plugin_handle(), index, &mut buffer)?;

		Some(Self {
			id: param.id,
			flags: param.flags,
			cookie: param.cookie,
			name: str::from_utf8(param.name).ok()?.into(),
			range: param.min_value as f32..=param.max_value as f32,
			reset: param.default_value as f32,
			value: param.default_value as f32,
			value_text: None,
			index,
		})
	}

	pub fn rescan(&mut self, plugin: &mut PluginInstance<Host>, flags: ParamRescanFlags) {
		let ext = plugin.access_shared_handler(|s| *s.ext.params.get().unwrap());

		if flags.contains(ParamRescanFlags::INFO)
			&& let Some(param) = Self::try_new(plugin, self.index)
		{
			self.name = param.name;
			self.flags = param.flags;
		}

		if flags.contains(ParamRescanFlags::VALUES)
			&& let Some(value) = ext.get_value(&mut plugin.plugin_handle(), self.id)
		{
			self.value = value as f32;
		}

		if (flags.contains(ParamRescanFlags::VALUES) || flags.contains(ParamRescanFlags::TEXT))
			&& let Ok(value_text) = ext.value_to_text(
				&mut plugin.plugin_handle(),
				self.id,
				f64::from(self.value),
				&mut [MaybeUninit::zeroed(); 32],
			) && let Ok(value_text) = str::from_utf8(value_text)
			&& !value_text.is_empty()
		{
			self.value_text = Some(value_text.into());
		}
	}
}
