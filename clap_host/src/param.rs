use crate::host::Host;
use clack_extensions::{
	params::{ParamInfoBuffer, ParamInfoFlags, ParamRescanFlags},
	state::HostStateImpl as _,
};
use clack_host::{prelude::*, utils::Cookie};
use std::{ops::RangeInclusive, sync::Arc};

#[derive(Debug)]
pub struct Param {
	pub id: ClapId,
	pub flags: ParamInfoFlags,
	pub cookie: Cookie,
	pub name: Arc<str>,
	pub module: Arc<str>,
	pub range: RangeInclusive<f32>,
	pub value: f32,
	pub value_text: Option<Arc<str>>,
	pub default: f32,
	index: u32,
}

impl Param {
	pub fn all(plugin: &mut PluginInstance<Host>) -> Option<Box<[Self]>> {
		let params = *plugin.access_shared_handler(|s| s.ext.params.get())?;

		let count = params.count(&mut plugin.plugin_handle());

		let mut params = (0..count)
			.filter_map(|index| Self::try_new(plugin, index))
			.collect::<Box<_>>();

		for param in &mut params {
			param.rescan(plugin, ParamRescanFlags::VALUES);
		}

		Some(params)
	}

	fn try_new(plugin: &mut PluginInstance<Host>, index: u32) -> Option<Self> {
		let params = *plugin.access_shared_handler(|s| s.ext.params.get())?;
		let mut buffer = ParamInfoBuffer::new();
		let param = params.get_info(&mut plugin.plugin_handle(), index, &mut buffer)?;

		Some(Self {
			id: param.id,
			flags: param.flags,
			cookie: param.cookie,
			name: str::from_utf8(param.name).ok()?.into(),
			module: str::from_utf8(param.module).ok()?.into(),
			range: param.min_value as f32..=param.max_value as f32,
			value: param.default_value as f32,
			value_text: None,
			default: param.default_value as f32,
			index,
		})
	}

	pub fn rescan(&mut self, plugin: &mut PluginInstance<Host>, flags: ParamRescanFlags) {
		let params = plugin.access_shared_handler(|s| *s.ext.params.get().unwrap());

		if flags.contains(ParamRescanFlags::INFO)
			&& let Some(param) = Self::try_new(plugin, self.index)
		{
			self.name = param.name;
			self.module = param.module;
			self.flags = param.flags;
		}

		if flags.contains(ParamRescanFlags::VALUES)
			&& let Some(value) = params.get_value(&mut plugin.plugin_handle(), self.id)
		{
			self.value = value as f32;
			plugin.access_handler_mut(|mt| mt.mark_dirty());
		}

		if (flags.contains(ParamRescanFlags::VALUES) || flags.contains(ParamRescanFlags::TEXT))
			&& let Ok(value_text) = params.value_to_text(
				&mut plugin.plugin_handle(),
				self.id,
				self.value.into(),
				&mut [0; 256],
			) && let Ok(value_text) = str::from_utf8(value_text)
			&& !value_text.is_empty()
		{
			self.value_text = Some(value_text.into());
		}
	}

	pub fn adjust_value(&mut self, plugin: &mut PluginInstance<Host>, value: f32) -> bool {
		if self.flags.contains(ParamInfoFlags::IS_READONLY) {
			return false;
		}

		self.value = value;
		plugin.access_handler_mut(|mt| mt.mark_dirty());
		self.rescan(plugin, ParamRescanFlags::TEXT);

		true
	}
}
