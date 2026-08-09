use crate::{EventImpl, NodeId, ScratchImpl};
use std::{convert::Infallible, marker::PhantomData};
use thread_pool::{Erased, WorkList};

pub type Injector<Node> = thread_pool::Injector<Inject<Node>>;

#[derive(Debug)]
pub struct Inject<Node: NodeImpl>(PhantomData<Node>);

impl<Node: NodeImpl> Erased for Inject<Node> {
	type Scratch = ();
	type Inject = Infallible;
	type WorkList<'a> = Node::Inject<'a>;
}

pub trait NodeImpl: Send + Sized + 'static {
	type Event: EventImpl;
	type State: Sync;
	type Scratch: ScratchImpl;
	type Inject<'a>: WorkList<Scratch = (), Inject = Infallible>;
	#[must_use]
	fn process(
		&mut self,
		state: &Self::State,
		audio: &mut [[f32; 2]],
		events: &mut Vec<Self::Event>,
		scratch: &mut Self::Scratch,
		injector: &Injector<Self>,
	) -> usize;
	#[must_use]
	fn id(&self) -> NodeId;
	fn reset(&mut self);
}
