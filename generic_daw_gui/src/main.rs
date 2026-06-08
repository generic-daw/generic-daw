#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use daw::{CRASHES_DIR, Daw, format_now};
use iced::{Result, daemon};
use icons::LUCIDE_BYTES;
use log::LevelFilter;
use std::{backtrace::Backtrace, fs::File, io::Write as _};

mod action;
mod arrangement_view;
mod clap_host;
mod components;
mod config;
mod config_view;
mod daw;
mod file_tree;
mod icons;
mod lod;
mod operation;
mod state;
mod stylefns;
mod theme;
mod widget;

fn main() -> Result {
	install_panic_hook();

	env_logger::builder()
		.filter_level(LevelFilter::Warn)
		.parse_default_env()
		.init();

	daemon(Daw::create, Daw::update, Daw::view)
		.title(Daw::title)
		.theme(Daw::theme)
		.scale_factor(Daw::scale_factor)
		.subscription(Daw::subscription)
		.font(LUCIDE_BYTES)
		.run()
}

fn install_panic_hook() {
	let default_hook = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |info| {
		default_hook(info);
		if let Ok(mut file) = File::create(CRASHES_DIR.join(format!("{}.log", format_now()))) {
			let current_thread = std::thread::current();
			let current_thread_name = current_thread.name().unwrap_or("<unnamed>");
			let backtrace = Backtrace::force_capture();
			_ = write!(
				file,
				"thread '{current_thread_name}' {info}\nstack backtrace:\n{backtrace}",
			);
		}
	}));
}
