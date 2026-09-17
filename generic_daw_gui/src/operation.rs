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
				(s.unwrap(), scrollable.clone(), child.clone())
			}),
			|(s, scrollable, child)| {
				operation::map(child.find(), move |c| {
					(s.clone(), c.unwrap(), scrollable.clone())
				})
			},
		),
		|(s, c, scrollable)| {
			let selector::Target::Scrollable { translation, .. } = s else {
				panic!();
			};

			scrollable::scroll_to(
				scrollable,
				scrollable::AbsoluteOffset {
					x: c.visible_bounds()
						.is_none_or(|vb| vb.width != c.bounds().width)
						.then(|| {
							c.bounds().x - s.bounds().x
								+ if c.bounds().x - s.bounds().x < translation.x {
									0.0
								} else {
									c.bounds().width - s.bounds().width
								}
						}),
					y: c.visible_bounds()
						.is_none_or(|vb| vb.height != c.bounds().height)
						.then(|| {
							c.bounds().y - s.bounds().y
								+ if c.bounds().y - s.bounds().y < translation.y {
									0.0
								} else {
									c.bounds().height - s.bounds().height
								}
						}),
				},
			)
		},
	))
}
