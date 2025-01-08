use super::{
    gui::GuiExt,
    host::{Host, HostThreadMessage},
    main_thread::MainThreadMessage,
};
use clack_host::{
    plugin::PluginInstance,
    prelude::{AudioPorts, EventBuffer},
};
use std::{
    fmt::Debug,
    sync::mpsc::{Receiver, Sender},
};
use winit::dpi::Size;

pub struct ClapPlugin {
    instance: PluginInstance<Host>,
    gui: GuiExt,
    sender: Sender<MainThreadMessage>,
    receiver: Receiver<HostThreadMessage>,
}

impl Debug for ClapPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClapPlugin").finish_non_exhaustive()
    }
}

impl ClapPlugin {
    pub fn new(
        instance: PluginInstance<Host>,
        gui: GuiExt,
        sender: Sender<MainThreadMessage>,
        receiver: Receiver<HostThreadMessage>,
    ) -> Self {
        Self {
            instance,
            gui,
            sender,
            receiver,
        }
    }

    pub fn process_audio(
        &self,
        input_audio: Vec<Vec<f32>>,
        input_audio_ports: AudioPorts,
        output_audio_ports: AudioPorts,
        input_events: EventBuffer,
    ) -> (Vec<Vec<f32>>, EventBuffer) {
        self.sender
            .send(MainThreadMessage::ProcessAudio(
                input_audio,
                input_audio_ports,
                output_audio_ports,
                input_events,
            ))
            .unwrap();

        match self.receiver.recv() {
            Ok(HostThreadMessage::AudioProcessed(output_audio, output_events)) => {
                (output_audio, output_events)
            }
            _ => unreachable!(),
        }
    }

    pub fn get_counter(&self) -> u64 {
        self.sender.send(MainThreadMessage::GetCounter).unwrap();

        match self.receiver.recv() {
            Ok(HostThreadMessage::Counter(counter)) => counter,
            _ => unreachable!(),
        }
    }

    pub fn get_state(&self) -> Vec<u8> {
        self.sender.send(MainThreadMessage::GetState).unwrap();

        match self.receiver.recv() {
            Ok(HostThreadMessage::State(state)) => state,
            _ => unreachable!(),
        }
    }

    pub fn set_state(&self, state: Vec<u8>) {
        self.sender
            .send(MainThreadMessage::SetState(state))
            .unwrap();
    }

    pub fn resize(&mut self, size: iced::Size) {
        self.gui.resize(
            &mut self.instance.plugin_handle(),
            Size::Physical(winit::dpi::PhysicalSize::new(
                size.width as u32,
                size.height as u32,
            )),
        );
    }

    pub fn destroy(mut self) {
        self.gui.destroy(&mut self.instance.plugin_handle());
    }
}
