use iced::{
	Task,
	advanced::graphics::futures::MaybeSend,
	widget::{self, operation, selector},
};

pub fn scroll_into_view<T: MaybeSend + 'static>(
	scrollable: impl Into<widget::Id>,
	child: impl Into<widget::Id>,
) -> Task<T> {
	let scrollable = scrollable.into();
	let child = child.into();

	selector::find(scrollable.clone()).and_then(move |s| {
		let scrollable = scrollable.clone();
		selector::find(child.clone()).and_then(move |c| {
			operation::scroll_to(
				scrollable.clone(),
				operation::AbsoluteOffset {
					x: c.visible_bounds()
						.is_none_or(|bounds| bounds.width != c.bounds().width)
						.then_some(
							c.bounds().x - s.bounds().width
								+ if c.bounds().x < s.bounds().x {
									0.0
								} else {
									c.bounds().width - s.bounds().x
								},
						),
					y: c.visible_bounds()
						.is_none_or(|bounds| bounds.height != c.bounds().height)
						.then_some(
							c.bounds().y - s.bounds().height
								+ if c.bounds().y < s.bounds().y {
									0.0
								} else {
									c.bounds().height - s.bounds().y
								},
						),
				},
			)
		})
	})
}
