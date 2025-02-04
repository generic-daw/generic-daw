use generic_daw_core::rtrb::Consumer;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

#[derive(Debug)]
pub struct AsyncConsumer<T>(pub Consumer<T>);

impl<T> Future for AsyncConsumer<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        cx.waker().wake_by_ref();
        self.0.pop().map_or(Poll::Pending, Poll::Ready)
    }
}
