use crate::{AudioGraph, EventImpl, NodeId};
use std::{convert::Infallible, marker::PhantomData};
use thread_pool::{Erased, WorkList};

pub type ThreadPool<Node> = thread_pool::ThreadPool<AudioGraph<Node>>;
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
	type Scratch: Send;
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
	fn get_scratch_audio(scratch: &mut Self::Scratch) -> &mut [[f32; 2]];
	#[must_use]
	fn id(&self) -> NodeId;
	fn reset(&mut self);
}
