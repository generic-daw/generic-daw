#![expect(missing_debug_implementations)]

use crossbeam_utils::{Backoff, CachePadded};
use std::{
	convert::Infallible,
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

pub trait WorkList: Sync {
	type Item;
	type Scratch: Send;
	type Inject: Erased<Scratch = (), Inject = Infallible>;
	#[must_use]
	fn next_item(&self) -> Option<Self::Item>;
	#[must_use]
	fn do_work(
		&self,
		item: Self::Item,
		scratch: &mut Self::Scratch,
		injector: &Injector<Self::Inject>,
	) -> Option<Self::Item>;
}

pub trait Erased: 'static {
	type Scratch: Send;
	type Inject: Erased<Scratch = (), Inject = Infallible>;
	type WorkList<'a>: WorkList<Scratch = Self::Scratch, Inject = Self::Inject>;
}

impl<T: WorkList + 'static> Erased for T {
	type Scratch = <T as WorkList>::Scratch;
	type Inject = <T as WorkList>::Inject;
	type WorkList<'a> = T;
}

impl WorkList for Infallible {
	type Item = Self;
	type Scratch = ();
	type Inject = Self;

	fn next_item(&self) -> Option<Self::Item> {
		None
	}

	fn do_work(
		&self,
		_item: Self::Item,
		_scratch: &mut Self::Scratch,
		_injector: &Injector<Self::Inject>,
	) -> Option<Self::Item> {
		None
	}
}

pub struct ThreadPool<W: Erased<Inject: Erased<Scratch = (), Inject = Infallible>>> {
	shared: Arc<Shared<W>>,
	threads: Box<[JoinHandle<()>]>,
	scratch: W::Scratch,
}

impl<W: Erased<Scratch = (), Inject: Erased<Scratch = (), Inject = Infallible>>> Default
	for ThreadPool<W>
{
	fn default() -> Self {
		Self::new()
	}
}

impl<W: Erased<Scratch = (), Inject: Erased<Scratch = (), Inject = Infallible>>> ThreadPool<W> {
	#[must_use]
	pub fn new() -> Self {
		Self::new_with_threads(Self::default_threads())
	}

	#[must_use]
	pub fn new_with_threads(threads: NonZero<usize>) -> Self {
		Self::new_with_threads_and_scratch(threads, || ())
	}
}

impl<W: Erased<Inject: Erased<Scratch = (), Inject = Infallible>>> ThreadPool<W> {
	#[must_use]
	pub fn default_threads() -> NonZero<usize> {
		if let Ok(threads) = available_parallelism()
			&& let Some(threads) = NonZero::new(threads.get() - 1)
		{
			threads
		} else {
			NonZero::new(1).unwrap()
		}
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
			main: CachePadded::new(Injector::new()),
			inject: CachePadded::new(Injector::new()),
		});

		let scratch = make_scratch();

		let threads = (1..threads.get())
			.map(|i| {
				let shared = shared.clone();
				let scratch = make_scratch();
				std::thread::Builder::new()
					.name(format!("worker-{i}"))
					.spawn(move || shared.main.worker(scratch, &shared.inject))
					.unwrap()
			})
			.collect();

		Self {
			shared,
			threads,
			scratch,
		}
	}

	#[must_use]
	pub fn threads(&self) -> NonZero<usize> {
		NonZero::new(self.threads.len() + 1).unwrap()
	}

	pub fn run(&mut self, work_list: &W::WorkList<'_>, to_do: usize, threads: NonZero<usize>) {
		assert!(self.shared.main.try_install(work_list, to_do));

		for thread in self.threads.iter().take(threads.get() - 1) {
			thread.thread().unpark();
		}

		self.shared
			.main
			.do_work::<true>(work_list, &mut self.scratch, &self.shared.inject);

		self.shared.main.join();
	}
}

impl<W: Erased<Inject: Erased<Scratch = (), Inject = Infallible>>> Drop for ThreadPool<W> {
	fn drop(&mut self) {
		self.shared.main.stop.store(true, Relaxed);
		self.shared.main.epoch.fetch_add(1, Release);

		for thread in &self.threads {
			thread.thread().unpark();
		}

		for thread in std::mem::take(&mut self.threads) {
			thread.join().unwrap();
		}
	}
}

struct Shared<W: Erased<Inject: Erased<Scratch = (), Inject = Infallible>>> {
	main: CachePadded<Injector<W>>,
	inject: CachePadded<Injector<W::Inject>>,
}

pub struct Injector<W: Erased> {
	work_list: AtomicPtr<W::WorkList<'static>>,
	epoch: AtomicUsize,
	active: AtomicUsize,
	to_do: AtomicUsize,
	stop: AtomicBool,
	mt_working: AtomicBool,
}

impl<W: Erased> Injector<W> {
	const fn new() -> Self {
		Self {
			work_list: AtomicPtr::new(std::ptr::null_mut()),
			epoch: AtomicUsize::new(0),
			active: AtomicUsize::new(usize::MAX),
			to_do: AtomicUsize::new(0),
			stop: AtomicBool::new(false),
			mt_working: AtomicBool::new(true),
		}
	}

	fn try_install(&self, work_list: &W::WorkList<'_>, to_do: usize) -> bool {
		if self
			.work_list
			.compare_exchange(
				std::ptr::null_mut(),
				std::ptr::from_ref(work_list).cast_mut().cast(),
				Acquire,
				Relaxed,
			)
			.is_err()
		{
			return false;
		}

		self.to_do.store(to_do, Relaxed);
		self.mt_working.store(true, Relaxed);

		assert_eq!(self.active.swap(0, Release), usize::MAX);

		self.epoch.fetch_add(1, Release);

		true
	}

	fn join(&self) {
		let backoff = Backoff::new();

		if self.active.fetch_sub(1, AcqRel) != 0 {
			backoff.spin();
			while self.active.load(Acquire) != usize::MAX {
				backoff.spin();
			}
		}

		debug_assert_eq!(self.to_do.load(Relaxed), 0);

		self.work_list.store(std::ptr::null_mut(), Release);
	}

	fn worker(&self, mut scratch: W::Scratch, injector: &Injector<W::Inject>) {
		let mut epoch = 0;
		loop {
			epoch = self.wait_for_work(epoch);

			if self.stop.load(Relaxed) {
				return;
			}

			self.try_do_work(&mut scratch, injector);
		}
	}

	fn wait_for_work(&self, last_epoch: usize) -> usize {
		let backoff = Backoff::new();
		loop {
			let new_epoch = self.epoch.load(Acquire);
			if new_epoch != last_epoch {
				return new_epoch;
			}

			if backoff.is_completed() {
				std::thread::park();

				backoff.reset();
			} else {
				backoff.snooze();
			}
		}
	}

	fn try_do_work(&self, scratch: &mut W::Scratch, injector: &Injector<W::Inject>) -> bool {
		if self
			.active
			.fetch_update(Acquire, Relaxed, |active| active.checked_add(1))
			.is_err()
		{
			return false;
		}

		// SAFETY:
		// `self.work_list` is a valid reference to a `W::WorkList`, because `try_install`
		// previously set it to a reference to a `W::WorkList` that is valid at least until `join`
		// finishes. `join` hasn't finished yet because `self.active` wasn't `usize::MAX` at the
		// `fetch_update`, and can't be `usize::MAX` again before the `fetch_sub`.
		let work_list = unsafe { self.work_list.load(Relaxed).as_ref() }.unwrap();

		self.do_work::<false>(work_list, scratch, injector);

		assert_ne!(self.active.fetch_sub(1, Release), usize::MAX);

		true
	}

	fn do_work<const MT: bool>(
		&self,
		work_list: &W::WorkList<'_>,
		scratch: &mut W::Scratch,
		injector: &Injector<W::Inject>,
	) {
		let backoff = Backoff::new();
		while self.to_do.load(Relaxed) != 0 {
			if (MT || self.mt_working.load(Relaxed))
				&& let Some(item) = work_list.next_item()
			{
				self.do_item_and_reserved::<MT>(work_list, item, scratch, injector);

				backoff.reset();
			} else if injector.try_do_work(&mut (), &Injector::new()) {
				backoff.reset();
			} else if MT {
				backoff.spin();
			} else if backoff.is_completed() {
				break;
			} else {
				backoff.snooze();
			}
		}
	}

	fn do_item_and_reserved<'a, const MT: bool>(
		&self,
		work_list: &W::WorkList<'a>,
		mut item: <W::WorkList<'a> as WorkList>::Item,
		scratch: &mut W::Scratch,
		injector: &Injector<W::Inject>,
	) {
		if MT {
			self.mt_working.store(true, Relaxed);
		}

		loop {
			let to_do = self.to_do.fetch_sub(1, Relaxed);
			debug_assert_ne!(to_do, 0);

			if let Some(reserved) = work_list.do_work(item, scratch, injector) {
				item = reserved;
			} else {
				break;
			}
		}

		if MT {
			self.mt_working.store(false, Relaxed);
		}
	}
}

impl<W: Erased<Scratch = (), Inject = Infallible>> Injector<W> {
	pub fn inject(&self, work_list: &W::WorkList<'_>, mut to_do: usize) {
		let mut reserved = None;
		while to_do != 0 {
			if to_do + usize::from(reserved.is_some()) != 1 && self.try_install(work_list, to_do) {
				if let Some(item) = reserved {
					self.do_item_and_reserved::<true>(work_list, item, &mut (), &Injector::new());
				}

				self.do_work::<true>(work_list, &mut (), &Injector::new());

				self.join();

				return;
			} else if let Some(item) = reserved.take().or_else(|| work_list.next_item()) {
				to_do -= 1;
				reserved = work_list.do_work(item, &mut (), &Injector::new());
			} else {
				debug_assert!(false);
			}
		}
	}
}
