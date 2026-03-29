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

	selector::find(scrollable.clone())
		.and_then(move |s| selector::find(child.clone()).map(move |c| c.map(|c| (s.clone(), c))))
		.and_then(move |(s, c)| {
			let selector::Target::Scrollable { translation, .. } = s else {
				panic!();
			};

			operation::scroll_to(
				scrollable.clone(),
				operation::AbsoluteOffset {
					x: c.visible_bounds()
						.is_none_or(|vb| vb.width != c.bounds().width)
						.then_some(
							c.bounds().x - s.bounds().x
								+ if c.bounds().x - s.bounds().x < translation.x {
									0.0
								} else {
									c.bounds().width - s.bounds().width
								},
						),
					y: c.visible_bounds()
						.is_none_or(|vb| vb.height != c.bounds().height)
						.then_some(
							c.bounds().y - s.bounds().y
								+ if c.bounds().y - s.bounds().y < translation.y {
									0.0
								} else {
									c.bounds().height - s.bounds().height
								},
						),
				},
			)
		})
}
