use super::{timer::Timers, AudioProcessor, Host, HostThreadMessage, MainThreadMessage};
use clack_extensions::{
    gui::{GuiApiType, GuiConfiguration, GuiSize, PluginGui, Window as ClapWindow},
    state::PluginState,
    timer::PluginTimer,
};
use clack_host::prelude::*;
use std::{
    io::Cursor,
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};
use winit::{
    dpi::{LogicalSize, PhysicalSize, Size},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
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

    pub fn open_floating(&self, plugin: &mut PluginMainThreadHandle<'_>) {
        let Some(configuration) = self.configuration else {
            panic!("Called open_floating on incompatible plugin")
        };
        assert!(
            configuration.is_floating,
            "Called open_floating on incompatible plugin"
        );

        self.plugin_gui.create(plugin, configuration).unwrap();
        self.plugin_gui.suggest_title(plugin, c"");
        self.plugin_gui.show(plugin).unwrap();
    }

    pub fn open_embedded(
        &mut self,
        plugin: &mut PluginMainThreadHandle<'_>,
        event_loop: &EventLoop<()>,
    ) -> Window {
        let Some(configuration) = self.configuration else {
            panic!("Called open_embedded on incompatible plugin")
        };
        assert!(
            !configuration.is_floating,
            "Called open_embedded on incompatible plugin"
        );

        self.plugin_gui.create(plugin, configuration).unwrap();

        let initial_size = self.plugin_gui.get_size(plugin).unwrap_or(GuiSize {
            width: 640,
            height: 480,
        });

        self.is_resizeable = self.plugin_gui.can_resize(plugin);

        #[expect(deprecated)] // TODO: remove this
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Clack CPAL plugin!")
                    .with_inner_size(PhysicalSize {
                        height: initial_size.height,
                        width: initial_size.width,
                    })
                    .with_resizable(self.is_resizeable),
            )
            .unwrap();

        unsafe {
            self.plugin_gui
                .set_parent(plugin, ClapWindow::from_window(&window).unwrap())
                .unwrap();
        }

        let _ = self.plugin_gui.show(plugin);
        self.is_open = true;

        window
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

    pub fn run_gui_embedded(
        &mut self,
        mut instance: PluginInstance<Host>,
        sender: &Sender<HostThreadMessage>,
        receiver: &Receiver<MainThreadMessage>,
        audio_processor: &mut AudioProcessor,
    ) {
        let event_loop = EventLoop::new().unwrap();

        let mut window = Some(self.open_embedded(&mut instance.plugin_handle(), &event_loop));

        let uses_logical_pixels = self.configuration.unwrap().api_type.uses_logical_size();

        let timers =
            instance.access_handler(|h| h.timer_support.map(|ext| (h.timers.clone(), ext)));

        #[expect(deprecated)]
        event_loop
            .run(move |event, target| {
                if let Some((timers, timer_ext)) = &timers {
                    timers.tick_timers(timer_ext, &mut instance.plugin_handle());
                }

                while let Ok(message) = receiver.try_recv() {
                    match message {
                        MainThreadMessage::GuiRequestResized(new_size) => {
                            let new_size: Size = if uses_logical_pixels {
                                LogicalSize {
                                    width: new_size.width,
                                    height: new_size.height,
                                }
                                .into()
                            } else {
                                PhysicalSize {
                                    width: new_size.width,
                                    height: new_size.height,
                                }
                                .into()
                            };

                            let _ = window.as_mut().unwrap().request_inner_size(new_size);
                        }
                        _ => Self::do_event(&mut instance, sender, message, audio_processor),
                    }
                }

                match event {
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => {
                            self.plugin_gui.destroy(&mut instance.plugin_handle());
                            window.take();
                            return;
                        }
                        WindowEvent::Destroyed => {
                            target.exit();
                            return;
                        }
                        WindowEvent::Resized(size) => {
                            let window = window.as_ref().unwrap();
                            let scale_factor = window.scale_factor();

                            let actual_size = self.resize(
                                &mut instance.plugin_handle(),
                                Size::Physical(size),
                                scale_factor,
                            );

                            if actual_size != size.into() {
                                let _ = window.request_inner_size(actual_size);
                            }
                        }
                        _ => {}
                    },
                    Event::LoopExiting => {
                        self.plugin_gui.destroy(&mut instance.plugin_handle());
                    }
                    _ => {}
                }

                let sleep_duration = Self::get_sleep_duration(timers.as_ref());

                target.set_control_flow(ControlFlow::WaitUntil(Instant::now() + sleep_duration));
            })
            .unwrap();

        std::thread::sleep(Duration::from_millis(100));
    }

    pub fn run_gui_floating(
        &mut self,
        mut instance: PluginInstance<Host>,
        sender: &Sender<HostThreadMessage>,
        receiver: &Receiver<MainThreadMessage>,
        audio_processor: &mut AudioProcessor,
    ) {
        self.open_floating(&mut instance.plugin_handle());
        let timers =
            instance.access_handler(|h| h.timer_support.map(|ext| (h.timers.clone(), ext)));

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
                    _ => Self::do_event(&mut instance, sender, message, audio_processor),
                }
            }

            let sleep_duration = Self::get_sleep_duration(timers.as_ref());

            std::thread::sleep(sleep_duration);
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
