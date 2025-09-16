use crate::NoDebug;
use std::cmp::Ordering;

#[derive(Clone, Debug)]
pub struct AudioRingbuf {
	buf: NoDebug<Vec<f32>>,
	head: usize,
}

impl AudioRingbuf {
	#[must_use]
	pub fn new(len: usize) -> Self {
		Self {
			buf: vec![0.0; len].into(),
			head: 0,
		}
	}

	pub fn next(&mut self, buf: &mut [f32]) {
		let diff = self.buf.len() - self.head;
		if self.buf.len() < buf.len() {
			buf.rotate_right(self.buf.len());
			buf[..self.buf.len()][..diff].swap_with_slice(&mut self.buf[self.head..]);
			buf[..self.buf.len()][diff..].swap_with_slice(&mut self.buf[..self.head]);
		} else if diff < buf.len() {
			self.buf[self.head..].swap_with_slice(&mut buf[..diff]);
			self.head = buf.len() - diff;
			self.buf[..self.head].swap_with_slice(&mut buf[diff..]);
		} else {
			self.buf[self.head..][..buf.len()].swap_with_slice(buf);
			self.head += buf.len();
		}
	}

	#[must_use]
	pub fn len(&self) -> usize {
		self.buf.len()
	}

	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.buf.is_empty()
	}

	pub fn resize(&mut self, len: usize) {
		match len.cmp(&self.buf.len()) {
			Ordering::Equal => return,
			Ordering::Greater => {
				self.buf.rotate_left(self.head);
				self.head = self.buf.len();
			}
			Ordering::Less => {
				if len < self.head {
					self.buf.rotate_left(self.head - len);
				} else {
					self.buf.rotate_right(len - self.head);
				}
				self.head = 0;
			}
		}

		self.buf.resize(len, 0.0);
	}
}
