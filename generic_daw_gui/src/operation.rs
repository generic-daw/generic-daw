use iced::{
	Task,
	advanced::{
		graphics::futures::MaybeSend,
		widget::{
			self,
			operation::{self, scrollable},
		},
	},
	widget::selector::{self, Selector as _},
};

pub fn scroll_into_view<T: MaybeSend + 'static>(
	scrollable: impl Into<widget::Id>,
	child: impl Into<widget::Id>,
) -> Task<T> {
	let scrollable = scrollable.into();
	let child = child.into();

	widget::operate(operation::then(
		operation::then(
			operation::map(scrollable.clone().find(), move |s| {
				(s, scrollable.clone(), child.clone())
			}),
			|(s, scrollable, child)| {
				operation::map(child.find(), move |c| (s.clone(), c, scrollable.clone()))
			},
		),
		|(s, c, scrollable)| {
			let t = s.as_ref().and_then(|s| match s {
				selector::Target::Scrollable { translation, .. } => Some(translation),
				_ => None,
			});

			scrollable::scroll_to(
				scrollable,
				scrollable::AbsoluteOffset {
					x: c.as_ref().zip(s.as_ref()).zip(t).and_then(|((c, s), t)| {
						c.visible_bounds()
							.is_none_or(|vb| vb.width != c.bounds().width)
							.then(|| {
								c.bounds().x - s.bounds().x
									+ if c.bounds().x - s.bounds().x < t.x {
										0.0
									} else {
										c.bounds().width - s.bounds().width
									}
							})
					}),
					y: c.as_ref().zip(s.as_ref()).zip(t).and_then(|((c, s), t)| {
						c.visible_bounds()
							.is_none_or(|vb| vb.height != c.bounds().height)
							.then(|| {
								c.bounds().y - s.bounds().y
									+ if c.bounds().y - s.bounds().y < t.y {
										0.0
									} else {
										c.bounds().height - s.bounds().height
									}
							})
					}),
				},
			)
		},
	))
}
