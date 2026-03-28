use crate::{MediaSource, Transport, resampler::Resampler};
use std::{
	fs::{self, File},
	io::{BufWriter, Read, Seek, SeekFrom, Write},
	num::NonZero,
	path::{Path, PathBuf},
	sync::{
		Arc, OnceLock,
		atomic::{AtomicBool, AtomicU64, Ordering},
		mpsc,
	},
	thread::{self, JoinHandle},
};
use symphonia::core::{
	audio::SampleBuffer,
	codecs::DecoderOptions,
	formats::FormatOptions,
	io::{MediaSourceStream, MediaSourceStreamOptions},
	meta::MetadataOptions,
	probe::Hint,
};
use utils::unique_id;

unique_id!(sample_id);

pub use sample_id::Id as SampleId;

const PAGE_SAMPLES: usize = 1 << 14;
const SAMPLE_CACHE_DIR: &str = "generic-daw-sample-cache";

static NEXT_CACHE_FILE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct Sample {
	pub id: SampleId,
	len: usize,
	cache_path: Arc<Path>,
	stream: StreamedSample,
}

#[derive(Debug)]
struct StreamedSample {
	len: usize,
	pages: Arc<[SamplePage]>,
	requests: Option<mpsc::Sender<usize>>,
	worker: Option<JoinHandle<()>>,
}

#[derive(Debug, Default)]
struct SamplePage {
	loaded: OnceLock<Box<[f32]>>,
	queued: AtomicBool,
}

impl Sample {
	#[must_use]
	pub fn new(source: Box<dyn MediaSource>, transport: &Transport) -> Option<Self> {
		Self::new_with_callback(source, transport, |_, _| {})
	}

	#[must_use]
	pub fn new_with_callback(
		source: Box<dyn MediaSource>,
		transport: &Transport,
		mut on_samples: impl FnMut(&[f32], usize),
	) -> Option<Self> {
		let cache_path = next_cache_path()?;
		let len = decode_to_cache(source, transport, &cache_path, &mut on_samples)?;

		Self::from_cache_path(SampleId::unique(), cache_path.into(), len)
	}

	#[must_use]
	pub fn from_samples(samples: Box<[f32]>) -> Option<Self> {
		let cache_path = next_cache_path()?;
		write_samples(&cache_path, &samples).ok()?;
		Self::from_cache_path(SampleId::unique(), cache_path.into(), samples.len())
	}

	#[must_use]
	pub fn len(&self) -> usize {
		self.len
	}

	#[must_use]
	pub fn cache_path(&self) -> &Path {
		&self.cache_path
	}

	pub fn mix_into(&self, offset: usize, audio: &mut [f32]) {
		self.stream.mix_into(offset, audio);
	}

	fn from_cache_path(id: SampleId, cache_path: Arc<Path>, len: usize) -> Option<Self> {
		Some(Self {
			id,
			len,
			cache_path: cache_path.clone(),
			stream: StreamedSample::new(cache_path, len)?,
		})
	}
}

impl StreamedSample {
	fn new(cache_path: Arc<Path>, len: usize) -> Option<Self> {
		let pages = (0..len.div_ceil(PAGE_SAMPLES))
			.map(|_| SamplePage::default())
			.collect::<Vec<_>>();
		let pages = Arc::<[SamplePage]>::from(pages);

		let (requests, receiver) = mpsc::channel();
		let worker_pages = pages.clone();
		let worker = thread::Builder::new()
			.name("sample-cache".into())
			.spawn(move || sample_worker(cache_path, len, worker_pages, receiver))
			.ok()?;

		Some(Self {
			len,
			pages,
			requests: Some(requests),
			worker: Some(worker),
		})
	}

	fn mix_into(&self, offset: usize, audio: &mut [f32]) {
		let len = self.len.saturating_sub(offset).min(audio.len());
		if len == 0 {
			return;
		}

		self.prefetch(offset, len);

		let mut written = 0;
		while written < len {
			let sample = offset + written;
			let page_idx = sample / PAGE_SAMPLES;
			let page_offset = sample % PAGE_SAMPLES;
			let chunk_len = (PAGE_SAMPLES - page_offset).min(len - written);

			if let Some(page) = self.pages[page_idx].loaded.get() {
				page[page_offset..page_offset + chunk_len]
					.iter()
					.zip(&mut audio[written..written + chunk_len])
					.for_each(|(sample, out)| *out += sample);
			}

			written += chunk_len;
		}
	}

	fn prefetch(&self, offset: usize, len: usize) {
		let first = offset / PAGE_SAMPLES;
		let last = (offset + len - 1) / PAGE_SAMPLES;

		self.request_page(first.saturating_sub(2));
		self.request_page(first.saturating_sub(1));
		for page in first..=last {
			self.request_page(page);
		}
		self.request_page(last + 1);
		self.request_page(last + 2);
	}

	fn request_page(&self, page_idx: usize) {
		let Some(page) = self.pages.get(page_idx) else {
			return;
		};
		if page.loaded.get().is_some() || page.queued.swap(true, Ordering::AcqRel) {
			return;
		}

		if let Some(requests) = &self.requests
			&& requests.send(page_idx).is_ok()
		{
			return;
		}

		page.queued.store(false, Ordering::Release);
	}
}

impl Drop for StreamedSample {
	fn drop(&mut self) {
		self.requests.take();

		if let Some(worker) = self.worker.take() {
			_ = worker.join();
		}
	}
}

fn next_cache_path() -> Option<PathBuf> {
	let dir = std::env::temp_dir().join(SAMPLE_CACHE_DIR);
	fs::create_dir_all(&dir).ok()?;

	Some(dir.join(format!(
		"{}.pcm",
		NEXT_CACHE_FILE.fetch_add(1, Ordering::Relaxed),
	)))
}

fn decode_to_cache(
	source: Box<dyn MediaSource>,
	transport: &Transport,
	cache_path: &Path,
	on_samples: &mut impl FnMut(&[f32], usize),
) -> Option<usize> {
	let mut format = symphonia::default::get_probe()
		.format(
			&Hint::default(),
			MediaSourceStream::new(source, MediaSourceStreamOptions::default()),
			&FormatOptions::default(),
			&MetadataOptions::default(),
		)
		.ok()?
		.format;

	let track = format.default_track()?;
	let track_id = track.id;
	let n_channels = track.codec_params.channels?.count();
	let delay = track.codec_params.delay.unwrap_or_default() as usize;
	let padding = track.codec_params.padding.unwrap_or_default() as usize;

	let mut resampler = Resampler::new(
		NonZero::new(track.codec_params.sample_rate?)?,
		transport.sample_rate,
		NonZero::new(2).unwrap(),
	)?
	.trim_start(delay)
	.trim_end(padding)
	.reserve(track.codec_params.n_frames.unwrap_or_default() as usize);

	let mut stereo = Vec::with_capacity(
		2 * track.codec_params.max_frames_per_packet.unwrap_or_default() as usize,
	);

	let mut writer = BufWriter::new(File::create(cache_path).ok()?);
	let mut decoder = symphonia::default::get_codecs()
		.make(&track.codec_params, &DecoderOptions::default())
		.ok()?;

	let mut sample_buf = None;
	while let Ok(packet) = format.next_packet() {
		if packet.track_id() != track_id {
			continue;
		}

		let audio_buf = decoder.decode(&packet).ok()?;
		let sample_buf = sample_buf.get_or_insert_with(|| {
			SampleBuffer::new(audio_buf.capacity() as u64, *audio_buf.spec())
		});
		sample_buf.copy_interleaved_ref(audio_buf.clone());

		if n_channels == 2 {
			stereo.extend(sample_buf.samples());
		} else if n_channels == 1 {
			stereo.extend(sample_buf.samples().iter().flat_map(|x| [x, x]));
		} else if n_channels != 0 {
			stereo.extend(
				sample_buf
					.samples()
					.chunks_exact(n_channels)
					.flat_map(|x| [x[0], x[1]]),
			);
		}

		let start = resampler.samples().len();
		resampler.process(&stereo);
		let new_samples = &resampler.samples()[start..];
		write_samples_to(&mut writer, new_samples).ok()?;
		on_samples(new_samples, start);

		stereo.clear();
	}

	let start = resampler.samples().len();
	let samples = resampler.finish();
	let tail = &samples[start..];
	write_samples_to(&mut writer, tail).ok()?;
	on_samples(tail, start);
	writer.flush().ok()?;

	Some(samples.len())
}

fn sample_worker(
	cache_path: Arc<Path>,
	len: usize,
	pages: Arc<[SamplePage]>,
	receiver: mpsc::Receiver<usize>,
) {
	let Ok(mut file) = File::open(&*cache_path) else {
		return;
	};

	while let Ok(page_idx) = receiver.recv() {
		let Some(page) = pages.get(page_idx) else {
			continue;
		};
		if page.loaded.get().is_some() {
			page.queued.store(false, Ordering::Release);
			continue;
		}

		let sample_offset = page_idx * PAGE_SAMPLES;
		let sample_len = PAGE_SAMPLES.min(len.saturating_sub(sample_offset));
		let result = read_page(&mut file, sample_offset, sample_len);

		if let Ok(samples) = result {
			_ = page.loaded.set(samples.into_boxed_slice());
		}

		page.queued.store(false, Ordering::Release);
	}
}

fn read_page(file: &mut File, sample_offset: usize, sample_len: usize) -> std::io::Result<Vec<f32>> {
	let mut bytes = vec![0; sample_len * size_of::<f32>()];
	file.seek(SeekFrom::Start((sample_offset * size_of::<f32>()) as u64))?;
	file.read_exact(&mut bytes)?;

	Ok(bytes
		.chunks_exact(size_of::<f32>())
		.map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
		.collect())
}

fn write_samples(path: &Path, samples: &[f32]) -> std::io::Result<()> {
	let mut writer = BufWriter::new(File::create(path)?);
	write_samples_to(&mut writer, samples)?;
	writer.flush()
}

fn write_samples_to(writer: &mut impl Write, samples: &[f32]) -> std::io::Result<()> {
	for &sample in samples {
		writer.write_all(&sample.to_le_bytes())?;
	}

	Ok(())
}
