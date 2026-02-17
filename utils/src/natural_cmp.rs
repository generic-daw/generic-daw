use std::cmp::Ordering;

pub fn natural_cmp(mut a: &[u8], mut b: &[u8]) -> Ordering {
	let mut a_cut;
	let mut b_cut;

	while !a.is_empty() || !b.is_empty() {
		(a_cut, a) = cut(a, u8::is_ascii_digit);
		(b_cut, b) = cut(b, u8::is_ascii_digit);

		match a_cut.cmp(b_cut) {
			Ordering::Equal => {}
			ord => return ord,
		}

		(_, a) = cut(a, |&c| c != b'0');
		(_, b) = cut(b, |&c| c != b'0');

		(a_cut, a) = cut(a, |c| !c.is_ascii_digit());
		(b_cut, b) = cut(b, |c| !c.is_ascii_digit());

		match a_cut.len().cmp(&b_cut.len()) {
			Ordering::Equal => {}
			ord => return ord,
		}

		match a_cut.cmp(b_cut) {
			Ordering::Equal => {}
			ord => return ord,
		}
	}

	Ordering::Equal
}

fn cut(s: &[u8], f: impl Fn(&u8) -> bool) -> (&[u8], &[u8]) {
	s.iter().position(f).map_or((s, &[]), |i| s.split_at(i))
}
