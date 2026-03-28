#[derive(Clone, Copy, Debug)]
pub enum MidiInputEvent {
	NoteOn { channel: u8, key: u8, velocity: f32 },
	NoteOff { channel: u8, key: u8, velocity: f32 },
}

#[cfg(target_os = "linux")]
mod imp {
	use super::MidiInputEvent;
	use alsa::{
		Direction,
		seq::{Addr, EvNote, EventType, Input, PortCap, PortIter, PortSubscribe, PortType, Seq},
	};
	use std::{
		ffi::CString,
		sync::{
			Arc,
			atomic::{AtomicBool, Ordering},
		},
		thread::{self, JoinHandle},
		time::Duration,
	};

	const CLIENT_NAME: &str = "Generic DAW";
	const PORT_NAME: &str = "Generic DAW MIDI Input";
	const IDLE_SLEEP: Duration = Duration::from_millis(5);

	#[derive(Debug)]
	struct MidiPort {
		name: Box<str>,
		addr: Addr,
	}

	#[derive(Debug)]
	pub struct MidiInputConnection {
		running: Arc<AtomicBool>,
		thread: Option<JoinHandle<()>>,
	}

	impl Drop for MidiInputConnection {
		fn drop(&mut self) {
			self.running.store(false, Ordering::Release);

			if let Some(thread) = self.thread.take() {
				let _ = thread.join();
			}
		}
	}

	pub fn get_midi_inputs() -> Box<[Box<str>]> {
		let Some(seq) = open_seq() else {
			return Box::default();
		};

		available_ports(&seq)
			.into_iter()
			.map(|port| port.name)
			.collect()
	}

	pub fn connect_midi_input(
		preferred_name: Option<&str>,
		mut on_event: impl FnMut(MidiInputEvent) + Send + 'static,
	) -> Option<MidiInputConnection> {
		let seq = open_seq()?;
		let ports = available_ports(&seq);
		let port = preferred_name
			.and_then(|preferred_name| {
				ports
					.iter()
					.find(|port| port.name.as_ref() == preferred_name)
			})
			.or_else(|| ports.first())?;

		let client_id = seq.client_id().ok()?;
		let client_name = CString::new(CLIENT_NAME).ok()?;
		let port_name = CString::new(PORT_NAME).ok()?;
		seq.set_client_name(&client_name).ok()?;

		let destination_port = seq
			.create_simple_port(
				&port_name,
				PortCap::WRITE | PortCap::SUBS_WRITE,
				PortType::MIDI_GENERIC | PortType::APPLICATION,
			)
			.ok()?;

		let destination = Addr {
			client: client_id,
			port: destination_port,
		};
		let source = port.addr;

		let subscription = PortSubscribe::empty().ok()?;
		subscription.set_sender(source);
		subscription.set_dest(destination);
		seq.subscribe_port(&subscription).ok()?;

		let running = Arc::new(AtomicBool::new(true));
		let thread_running = Arc::clone(&running);
		let thread = thread::Builder::new()
			.name(format!("{CLIENT_NAME} MIDI Input"))
			.spawn(move || {
				input_loop(
					seq,
					source,
					destination,
					&thread_running,
					&mut on_event,
				);
			})
			.ok()?;

		Some(MidiInputConnection {
			running,
			thread: Some(thread),
		})
	}

	fn open_seq() -> Option<Seq> {
		Seq::open(None, Some(Direction::Capture), true).ok()
	}

	fn available_ports(seq: &Seq) -> Vec<MidiPort> {
		let Some(client_id) = seq.client_id().ok() else {
			return Vec::new();
		};

		alsa::seq::ClientIter::new(seq)
			.flat_map(|client| {
				let client_name = client.get_name().ok().unwrap_or_default().to_owned();
				PortIter::new(seq, client.get_client())
					.filter(move |port| is_input_source(port, client_id))
					.filter_map(move |port| {
						let port_name = port.get_name().ok()?;
						Some(MidiPort {
							name: format!("{client_name} / {port_name}").into_boxed_str(),
							addr: port.addr(),
						})
					})
			})
			.collect()
	}

	fn is_input_source(port: &alsa::seq::PortInfo, client_id: i32) -> bool {
		port.get_client() != client_id
			&& port
				.get_capability()
				.contains(PortCap::READ | PortCap::SUBS_READ)
			&& port
				.get_type()
				.intersects(PortType::MIDI_GENERIC | PortType::APPLICATION | PortType::HARDWARE)
	}

	fn input_loop(
		seq: Seq,
		source: Addr,
		destination: Addr,
		running: &AtomicBool,
		on_event: &mut impl FnMut(MidiInputEvent),
	) {
		let mut input = seq.input();

		while running.load(Ordering::Acquire) {
			match input.event_input_pending(true) {
				Ok(0) => thread::sleep(IDLE_SLEEP),
				Ok(_) => drain_pending_events(&mut input, on_event),
				Err(_) => thread::sleep(IDLE_SLEEP),
			}
		}

		drop(input);
		_ = seq.unsubscribe_port(source, destination);
		_ = seq.delete_port(destination.port);
	}

	fn drain_pending_events(input: &mut Input<'_>, on_event: &mut impl FnMut(MidiInputEvent)) {
		loop {
			match input.event_input() {
				Ok(event) => {
					if let Some(event) = parse_event(&event) {
						on_event(event);
					}
				}
				Err(error) if error.errno() == libc::EAGAIN || error.errno() == libc::ENOSPC => {
					return;
				}
				Err(_) => return,
			}
		}
	}

	fn parse_event(event: &alsa::seq::Event<'_>) -> Option<MidiInputEvent> {
		let note = event.get_data::<EvNote>()?;
		let velocity = f32::from(note.velocity) / 127.0;

		match event.get_type() {
			EventType::Noteon if velocity > 0.0 => Some(MidiInputEvent::NoteOn {
				channel: note.channel,
				key: note.note,
				velocity,
			}),
			EventType::Noteon | EventType::Noteoff => Some(MidiInputEvent::NoteOff {
				channel: note.channel,
				key: note.note,
				velocity,
			}),
			_ => None,
		}
	}
}

#[cfg(target_os = "linux")]
pub use imp::{MidiInputConnection, connect_midi_input, get_midi_inputs};

#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
pub struct MidiInputConnection;

#[cfg(not(target_os = "linux"))]
pub fn get_midi_inputs() -> Box<[Box<str>]> {
	Box::default()
}

#[cfg(not(target_os = "linux"))]
pub fn connect_midi_input(
	_preferred_name: Option<&str>,
	_on_event: impl FnMut(MidiInputEvent) + Send + 'static,
) -> Option<MidiInputConnection> {
	None
}
