#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use daw::Daw;
use discovery::discover_plugins_send;
use iced::daemon;
use icons::LUCIDE_BYTES;
use log::{LevelFilter, error};

mod action;
mod arrangement_view;
mod clap_host;
mod components;
mod config;
mod config_view;
mod daw;
mod discovery;
mod file_tree;
mod icons;
mod lod;
mod state;
mod stylefns;
mod theme;
mod widget;

fn main() {
	env_logger::builder()
		.filter_module("clap_host", LevelFilter::Warn)
		.filter_module("generic_daw_core", LevelFilter::Warn)
		.filter_module("generic_daw", LevelFilter::Warn)
		.parse_default_env()
		.init();

	if std::env::args().any(|arg| arg == "--discover") {
		if let Err(Some(err)) = discover_plugins_send() {
			error!("{err}");
		}
	} else if let Err(err) = daemon(Daw::create, Daw::update, Daw::view)
		.title(Daw::title)
		.theme(Daw::theme)
		.scale_factor(Daw::scale_factor)
		.subscription(Daw::subscription)
		.antialiasing(true)
		.font(LUCIDE_BYTES)
		.run()
	{
		error!("{err}");
	}
}
