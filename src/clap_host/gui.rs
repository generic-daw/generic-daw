use super::{timer::Timers, AudioProcessor, Host, HostThreadMessage, MainThreadMessage};
use async_std::task;
use clack_extensions::{
    gui::{GuiApiType, GuiConfiguration, GuiSize, PluginGui, Window as ClapWindow},
    state::PluginState,
    timer::PluginTimer,
};
use clack_host::prelude::*;
use std::{
    future::Future,
    io::Cursor,
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};
use winit::{
    dpi::{LogicalSize, PhysicalSize, Size},
    raw_window_handle::RawWindowHandle,
};

pub struct GuiExt {
    plugin_gui: PluginGui,
    pub configuration: Option<GuiConfiguration<'static>>,
    is_open: bool,
    is_resizeable: bool,
}

impl GuiExt {
    pub fn new(plugin_gui: PluginGui, instance: &mut PluginMainThreadHandle<'_>) -> Self {
        Self {
            plugin_gui,
            configuration: Self::negotiate_configuration(&plugin_gui, instance),
            is_open: false,
            is_resizeable: false,
        }
    }

    fn negotiate_configuration(
        gui: &PluginGui,
        plugin: &mut PluginMainThreadHandle<'_>,
    ) -> Option<GuiConfiguration<'static>> {
        let api_type = GuiApiType::default_for_current_platform()?;
        let mut config = GuiConfiguration {
            api_type,
            is_floating: false,
        };

        if gui.is_api_supported(plugin, config) {
            Some(config)
        } else {
            config.is_floating = true;
            if gui.is_api_supported(plugin, config) {
                Some(config)
            } else {
                None
            }
        }
    }

    pub fn gui_size_to_winit_size(&self, size: GuiSize) -> Size {
        let Some(GuiConfiguration { api_type, .. }) = self.configuration else {
            panic!("Called gui_size_to_winit_size on incompatible plugin")
        };

        if api_type.uses_logical_size() {
            LogicalSize {
                width: size.width,
                height: size.height,
            }
            .into()
        } else {
            PhysicalSize {
                width: size.width,
                height: size.height,
            }
            .into()
        }
    }

    pub fn needs_floating(&self) -> Option<bool> {
        self.configuration
            .map(|GuiConfiguration { is_floating, .. }| is_floating)
    }

    pub fn open_floating(&mut self, plugin: &mut PluginMainThreadHandle<'_>) {
        let Some(configuration) = self.configuration.filter(|c| c.is_floating) else {
            panic!("Called open_floating on incompatible plugin")
        };

        self.plugin_gui.create(plugin, configuration).unwrap();
        self.plugin_gui.suggest_title(plugin, c"");
        self.plugin_gui.show(plugin).unwrap();

        self.is_resizeable = self.plugin_gui.can_resize(plugin);
        self.is_open = true;
    }

    pub fn open_embedded(
        &mut self,
        plugin: &mut PluginMainThreadHandle<'_>,
        window_handle: RawWindowHandle,
    ) {
        let Some(configuration) = self.configuration.filter(|c| !c.is_floating) else {
            panic!("Called open_embedded on incompatible plugin")
        };

        self.plugin_gui.create(plugin, configuration).unwrap();
        unsafe {
            self.plugin_gui
                .set_parent(
                    plugin,
                    ClapWindow::from_window_handle(window_handle).unwrap(),
                )
                .unwrap();
        }
        self.plugin_gui.show(plugin).unwrap();

        self.is_resizeable = self.plugin_gui.can_resize(plugin);
        self.is_open = true;
    }

    pub fn resize(
        &self,
        plugin: &mut PluginMainThreadHandle<'_>,
        size: Size,
        scale_factor: f64,
    ) -> Size {
        let uses_logical_pixels = self.configuration.unwrap().api_type.uses_logical_size();

        let size = if uses_logical_pixels {
            let size = size.to_logical(scale_factor);
            GuiSize {
                width: size.width,
                height: size.height,
            }
        } else {
            let size = size.to_physical(scale_factor);
            GuiSize {
                width: size.width,
                height: size.height,
            }
        };

        if !self.is_resizeable {
            let forced_size = self.plugin_gui.get_size(plugin).unwrap_or(size);

            return self.gui_size_to_winit_size(forced_size);
        }

        let working_size = self.plugin_gui.adjust_size(plugin, size).unwrap_or(size);
        self.plugin_gui.set_size(plugin, working_size).unwrap();

        self.gui_size_to_winit_size(working_size)
    }

    pub fn destroy(&mut self, plugin: &mut PluginMainThreadHandle<'_>) {
        if self.is_open {
            self.plugin_gui.destroy(plugin);
            self.is_open = false;
        }
    }

    #[expect(clippy::future_not_send)]
    pub fn run(
        mut self,
        mut instance: PluginInstance<Host>,
        sender: Sender<HostThreadMessage>,
        receiver: Receiver<MainThreadMessage>,
        mut audio_processor: AudioProcessor,
    ) -> impl Future {
        let timers =
            instance.access_handler(|h| h.timer_support.map(|ext| (h.timers.clone(), ext)));

        async move {
            loop {
                if let Some((timers, timer_ext)) = &timers {
                    timers.tick_timers(timer_ext, &mut instance.plugin_handle());
                }
                while let Ok(message) = receiver.try_recv() {
                    match message {
                        MainThreadMessage::GuiClosed { .. } => {
                            self.destroy(&mut instance.plugin_handle());
                            return;
                        }
                        MainThreadMessage::GuiRequestResized(new_size) => {
                            self.resize(
                                &mut instance.plugin_handle(),
                                self.gui_size_to_winit_size(new_size),
                                1.0f64,
                            );
                        }
                        _ => Self::do_event(&mut instance, &sender, message, &mut audio_processor),
                    }
                }

                let sleep_duration = Self::get_sleep_duration(timers.as_ref());
                task::sleep(sleep_duration).await;
            }
        }
    }

    fn do_event(
        instance: &mut PluginInstance<Host>,
        sender: &Sender<HostThreadMessage>,
        message: MainThreadMessage,
        audio_processor: &mut AudioProcessor,
    ) {
        match message {
            MainThreadMessage::RunOnMainThread => instance.call_on_main_thread_callback(),
            MainThreadMessage::ProcessAudio(
                mut input_buffers,
                mut input_audio_ports,
                mut output_audio_ports,
                input_events,
            ) => {
                let (output_buffers, output_events) = audio_processor.process(
                    &mut input_buffers,
                    &input_events,
                    &mut input_audio_ports,
                    &mut output_audio_ports,
                );

                sender
                    .send(HostThreadMessage::AudioProcessed(
                        output_buffers,
                        output_events,
                    ))
                    .unwrap();
            }
            MainThreadMessage::GetCounter => {
                sender
                    .send(HostThreadMessage::Counter(audio_processor.steady_time()))
                    .unwrap();
            }
            MainThreadMessage::GetState => {
                let state_ext: PluginState = instance
                    .access_handler_mut(|h| h.shared.state.get())
                    .unwrap()
                    .unwrap();

                let mut state = Vec::new();
                state_ext
                    .save(&mut instance.plugin_handle(), &mut state)
                    .unwrap();

                sender.send(HostThreadMessage::State(state)).unwrap();
            }
            MainThreadMessage::SetState(state) => {
                let state_ext: PluginState = instance
                    .access_handler_mut(|h| h.shared.state.get())
                    .unwrap()
                    .unwrap();

                let mut state = Cursor::new(state);

                state_ext
                    .load(&mut instance.plugin_handle(), &mut state)
                    .unwrap();
            }
            _ => unreachable!(),
        }
    }

    fn get_sleep_duration(timers: Option<&(Rc<Timers>, PluginTimer)>) -> Duration {
        timers
            .as_ref()
            .and_then(|(timers, _)| Some(timers.next_tick()? - Instant::now()))
            .unwrap_or(Duration::from_millis(30))
            .min(Duration::from_millis(30))
    }
}
