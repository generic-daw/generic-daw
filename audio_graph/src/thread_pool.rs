use crate::{AudioGraph, NodeImpl};
use crossbeam_utils::Backoff;
use std::{
	num::NonZero,
	sync::{
		Arc,
		atomic::{
			AtomicBool, AtomicPtr, AtomicUsize,
			Ordering::{AcqRel, Acquire, Relaxed, Release},
		},
	},
	thread::{JoinHandle, available_parallelism},
};
use utils::boxed_slice;

struct Shared<Node: NodeImpl> {
	audio_graph: AtomicPtr<AudioGraph<Node>>,
	state: AtomicPtr<Node::State>,
	epoch: AtomicUsize,
	active: AtomicUsize,
	to_do: AtomicUsize,
	stop: AtomicBool,
	mt_working: AtomicBool,
}

impl<Node: NodeImpl> Shared<Node> {
	fn install(&self, audio_graph: &AudioGraph<Node>, state: &Node::State, to_do: usize) {
		self.to_do.store(to_do, Relaxed);
		self.mt_working.store(true, Relaxed);

		self.audio_graph
			.store(std::ptr::from_ref(audio_graph).cast_mut(), Relaxed);
		self.state
			.store(std::ptr::from_ref(state).cast_mut(), Relaxed);

		assert_eq!(self.active.swap(0, Release), usize::MAX);

		self.epoch.fetch_add(1, Release);
	}

	fn join(&self) {
		let backoff = Backoff::new();

		if self.active.fetch_sub(1, AcqRel) != 0 {
			backoff.spin();
			while self.active.load(Acquire) != usize::MAX {
				backoff.spin();
			}
		}

		assert_eq!(self.to_do.load(Relaxed), 0);

		self.audio_graph.store(std::ptr::null_mut(), Release);
		self.state.store(std::ptr::null_mut(), Release);
	}

	fn worker(&self, frames: NonZero<u32>) {
		let mut scratch = boxed_slice![0.0; 2 * frames.get() as usize];

		let mut last_epoch = 0;

		let backoff = Backoff::new();
		loop {
			let new_epoch = self.epoch.load(Acquire);

			if new_epoch == last_epoch {
				if backoff.is_completed() {
					std::thread::park();

					backoff.reset();
				} else {
					backoff.snooze();
				}

				continue;
			}

			last_epoch = new_epoch;

			if self.stop.load(Relaxed) {
				return;
			}

			let Ok(_) = self
				.active
				.fetch_update(Acquire, Relaxed, |active| active.checked_add(1))
			else {
				continue;
			};

			backoff.reset();

			// `self.audio_graph` and `self.state` are a valid references to their respective types,
			// because `install` previously set them to references to their respective types that
			// are valid at least until `join` finishes. `join` hasn't finished yet because
			// `self.active` wasn't `usize::MAX` at the `fetch_update`, and can't be `usize::MAX`
			// again before the `fetch_sub`.

			// SAFETY: see above
			let audio_graph = unsafe { self.audio_graph.load(Relaxed).as_ref().unwrap() };

			// SAFETY: see above
			let state = unsafe { self.state.load(Relaxed).as_ref().unwrap() };

			self.do_work::<false>(audio_graph, state, &mut scratch);

			assert_ne!(self.active.fetch_sub(1, Release), usize::MAX);
		}
	}

	fn do_work<const MT: bool>(
		&self,
		audio_graph: &AudioGraph<Node>,
		state: &Node::State,
		scratch: &mut [f32],
	) {
		let backoff = Backoff::new();
		while self.to_do.load(Relaxed) != 0 {
			if (MT || self.mt_working.load(Relaxed))
				&& let Some(mut node) = audio_graph.next_node()
			{
				if MT {
					self.mt_working.store(true, Relaxed);
				}

				loop {
					assert_ne!(self.to_do.fetch_sub(1, Relaxed), 0);

					if let Some(inline) = audio_graph.process_node(node, state, scratch) {
						node = inline;
					} else {
						break;
					}
				}

				backoff.reset();

				if MT {
					self.mt_working.store(false, Relaxed);
				}
			} else if MT {
				backoff.spin();
			} else if backoff.is_completed() {
				break;
			} else {
				backoff.snooze();
			}
		}
	}
}

pub struct ThreadPool<Node: NodeImpl> {
	threads: Box<[JoinHandle<()>]>,
	shared: Arc<Shared<Node>>,
	scratch: Box<[f32]>,
}

impl<Node: NodeImpl> ThreadPool<Node> {
	pub fn new(frames: NonZero<u32>) -> Self {
		Self::with_threads(
			frames,
			available_parallelism().ok().or(NonZero::new(1)).unwrap(),
		)
	}

	pub fn with_threads(frames: NonZero<u32>, threads: NonZero<usize>) -> Self {
		let scratch = boxed_slice![0.0; 2 * frames.get() as usize];

		let shared = Arc::new(Shared {
			audio_graph: AtomicPtr::new(std::ptr::null_mut()),
			state: AtomicPtr::new(std::ptr::null_mut()),
			epoch: AtomicUsize::new(0),
			active: AtomicUsize::new(usize::MAX),
			to_do: AtomicUsize::new(0),
			stop: AtomicBool::new(false),
			mt_working: AtomicBool::new(false),
		});

		let threads = (1..threads.get())
			.map(|_| {
				std::thread::spawn({
					let shared = shared.clone();
					move || shared.worker(frames)
				})
			})
			.collect();

		Self {
			threads,
			shared,
			scratch,
		}
	}

	pub fn run(&mut self, audio_graph: &AudioGraph<Node>, state: &Node::State, to_do: usize) {
		self.shared.install(audio_graph, state, to_do);

		for thread in &self.threads {
			thread.thread().unpark();
		}

		self.shared
			.do_work::<true>(audio_graph, state, &mut self.scratch);

		self.shared.join();
	}
}

impl<Node: NodeImpl> Drop for ThreadPool<Node> {
	fn drop(&mut self) {
		self.shared.stop.store(true, Relaxed);
		self.shared.epoch.fetch_add(1, Release);

		for thread in &self.threads {
			thread.thread().unpark();
		}

		for thread in std::mem::take(&mut self.threads) {
			thread.join().unwrap();
		}
	}
}
