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
		let selector::Target::Scrollable { translation, .. } = s else {
			unreachable!();
		};

		let scrollable = scrollable.clone();
		selector::find(child.clone()).and_then(move |c| {
			operation::scroll_to(
				scrollable.clone(),
				operation::AbsoluteOffset {
					x: c.visible_bounds()
						.is_none_or(|vb| vb.width != c.bounds().width)
						.then_some(
							c.bounds().x - s.bounds().x - s.bounds().width
								+ if c.bounds().x - s.bounds().x < translation.x {
									s.bounds().width
								} else {
									c.bounds().width
								},
						),
					y: c.visible_bounds()
						.is_none_or(|vb| vb.height != c.bounds().height)
						.then_some(
							c.bounds().y - s.bounds().y - s.bounds().height
								+ if c.bounds().y - s.bounds().y < translation.y {
									s.bounds().height
								} else {
									c.bounds().height
								},
						),
				},
			)
		})
	})
}
