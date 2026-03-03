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

pub trait ErasedWorkList: 'static {
	type WorkList<'a>: WorkList<Scratch = Self::Scratch>;
	type Scratch: Send;
}

pub trait WorkList: Sync {
	type Item;
	type Scratch: Send;
	#[must_use]
	fn next_item(&self) -> Option<Self::Item>;
	#[must_use]
	fn do_work(&self, item: Self::Item, scratch: &mut Self::Scratch) -> Option<Self::Item>;
}

impl<T: WorkList + 'static> ErasedWorkList for T {
	type WorkList<'a> = T;
	type Scratch = <T as WorkList>::Scratch;
}

#[expect(missing_debug_implementations)]
pub struct ThreadPool<W: ErasedWorkList> {
	shared: Arc<Shared<W>>,
	threads: Box<[JoinHandle<()>]>,
	scratch: W::Scratch,
}

impl<W: ErasedWorkList<Scratch = ()>> Default for ThreadPool<W> {
	fn default() -> Self {
		Self::new()
	}
}

impl<W: ErasedWorkList<Scratch = ()>> ThreadPool<W> {
	#[must_use]
	pub fn new() -> Self {
		Self::new_with_threads(Self::default_threads())
	}

	#[must_use]
	pub fn new_with_threads(threads: NonZero<usize>) -> Self {
		Self::new_with_threads_and_scratch(threads, || ())
	}
}

impl<W: ErasedWorkList> ThreadPool<W> {
	#[must_use]
	pub fn default_threads() -> NonZero<usize> {
		available_parallelism().ok().or(NonZero::new(1)).unwrap()
	}

	#[must_use]
	pub fn new_with_scratch(make_scratch: impl FnMut() -> W::Scratch) -> Self {
		Self::new_with_threads_and_scratch(Self::default_threads(), make_scratch)
	}

	#[must_use]
	pub fn new_with_threads_and_scratch(
		threads: NonZero<usize>,
		mut make_scratch: impl FnMut() -> W::Scratch,
	) -> Self {
		let shared = Arc::new(Shared {
			work_list: AtomicPtr::new(std::ptr::null_mut()),
			epoch: AtomicUsize::new(0),
			active: AtomicUsize::new(usize::MAX),
			to_do: AtomicUsize::new(0),
			stop: AtomicBool::new(false),
			mt_working: AtomicBool::new(false),
		});

		let scratch = make_scratch();

		let threads = (1..threads.get())
			.map(|i| {
				let shared = shared.clone();
				let scratch = make_scratch();
				std::thread::Builder::new()
					.name(format!("worker-{i}"))
					.spawn(move || shared.worker(scratch))
					.unwrap()
			})
			.collect();

		Self {
			shared,
			threads,
			scratch,
		}
	}

	pub fn run(&mut self, work_list: &W::WorkList<'_>, to_do: usize) {
		self.shared.install(work_list, to_do);

		for thread in &self.threads {
			thread.thread().unpark();
		}

		self.shared.do_work::<true>(work_list, &mut self.scratch);

		self.shared.join();
	}
}

impl<W: ErasedWorkList> Drop for ThreadPool<W> {
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

struct Shared<W: ErasedWorkList> {
	work_list: AtomicPtr<W::WorkList<'static>>,
	epoch: AtomicUsize,
	active: AtomicUsize,
	to_do: AtomicUsize,
	stop: AtomicBool,
	mt_working: AtomicBool,
}

impl<W: ErasedWorkList> Shared<W> {
	fn install(&self, work_list: &W::WorkList<'_>, to_do: usize) {
		self.to_do.store(to_do, Relaxed);
		self.mt_working.store(true, Relaxed);

		self.work_list
			.store(std::ptr::from_ref(work_list).cast_mut().cast(), Relaxed);

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

		self.work_list.store(std::ptr::null_mut(), Release);
	}

	fn worker(&self, mut scratch: W::Scratch) {
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

			// SAFETY:
			// `self.work_list` is a valid reference to a `W::WorkList`, because `install`
			// previously set it to a reference to a `W::WorkList` that is valid at least until
			// `join` finishes. `join` hasn't finished yet because `self.active` wasn't `usize::MAX`
			// at the `fetch_update`, and can't be `usize::MAX` again before the `fetch_sub`.
			let work_list = unsafe { self.work_list.load(Relaxed).as_ref() }.unwrap();

			self.do_work::<false>(work_list, &mut scratch);

			assert_ne!(self.active.fetch_sub(1, Release), usize::MAX);
		}
	}

	fn do_work<const MT: bool>(&self, work_list: &W::WorkList<'_>, scratch: &mut W::Scratch) {
		let backoff = Backoff::new();
		while self.to_do.load(Relaxed) != 0 {
			if (MT || self.mt_working.load(Relaxed))
				&& let Some(mut work_item) = work_list.next_item()
			{
				if MT {
					self.mt_working.store(true, Relaxed);
				}

				loop {
					assert_ne!(self.to_do.fetch_sub(1, Relaxed), 0);

					if let Some(inline) = work_list.do_work(work_item, scratch) {
						work_item = inline;
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
