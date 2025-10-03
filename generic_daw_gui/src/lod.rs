pub const LOD_LEVELS: usize = 13;

pub fn create_lods(samples: &[f32]) -> [Box<[(f32, f32)]>; LOD_LEVELS] {
	let mut lods =
		std::array::from_fn(|i| Vec::with_capacity(samples.len().div_ceil(1 << (i + 3))));
	update_lods(samples, &mut lods, 0);
	lods.map(Vec::into_boxed_slice)
}

pub fn update_lods(samples: &[f32], lods: &mut [Vec<(f32, f32)>; LOD_LEVELS], mut start: usize) {
	start /= 8;

	lods[0].truncate(start);
	lods[0].extend(samples[start * 8..].chunks(8).map(|chunk| {
		let (min, max) = chunk
			.iter()
			.fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
				(min.min(c), max.max(c))
			});
		(min.mul_add(0.5, 0.5), max.mul_add(0.5, 0.5))
	}));

	for i in 1..LOD_LEVELS {
		let [last, current] = &mut lods[i - 1..=i] else {
			unreachable!()
		};

		start /= 2;
		current.truncate(start);
		current.extend(last[start * 2..].chunks(2).map(|chunk| {
			chunk
				.iter()
				.fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
					(min.min(c.0), max.max(c.1))
				})
		}));
	}
}
