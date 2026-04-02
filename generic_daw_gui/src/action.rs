use iced::{Task, advanced::graphics::futures::MaybeSend};

enum Storage<Instruction> {
	None,
	One(Instruction),
	Many(Vec<Instruction>),
}

pub struct Action<Instruction, Message> {
	instruction: Storage<Instruction>,
	task: Task<Message>,
}

impl<Instruction, Message> Action<Instruction, Message> {
	pub fn none() -> Self {
		Self {
			instruction: Storage::None,
			task: Task::none(),
		}
	}

	pub fn instruction(instruction: Instruction) -> Self {
		Self::none().with_instruction(instruction)
	}

	pub fn with_instruction(mut self, instruction: Instruction) -> Self {
		self.instruction = Storage::One(instruction);
		self
	}

	pub fn task(task: Task<Message>) -> Self {
		Self::none().with_task(task)
	}

	pub fn with_task(mut self, task: Task<Message>) -> Self {
		self.task = task;
		self
	}

	pub fn batch(actions: impl IntoIterator<Item = Self>) -> Self
	where
		Message: 'static,
	{
		actions.into_iter().fold(Self::none(), |acc, action| Self {
			instruction: match (acc.instruction, action.instruction) {
				(Storage::Many(mut l), Storage::Many(mut r)) => {
					l.append(&mut r);
					Storage::Many(l)
				}
				(Storage::Many(mut is), Storage::One(i))
				| (Storage::One(i), Storage::Many(mut is)) => {
					is.push(i);
					Storage::Many(is)
				}
				(Storage::One(l), Storage::One(r)) => Storage::Many(vec![l, r]),
				(s, Storage::None) | (Storage::None, s) => s,
			},
			task: Task::batch([acc.task, action.task]),
		})
	}

	pub fn handle<O: MaybeSend + 'static>(
		self,
		f1: impl FnMut(Message) -> O + MaybeSend + 'static,
		mut f2: impl FnMut(Instruction) -> Task<O>,
	) -> Task<O>
	where
		Message: MaybeSend + 'static,
	{
		Task::batch([
			self.task.map(f1),
			match self.instruction {
				Storage::None => Task::none(),
				Storage::One(instruction) => f2(instruction),
				Storage::Many(instructions) => Task::batch(instructions.into_iter().map(f2)),
			},
		])
	}
}

impl<Instruction, Message> From<Task<Message>> for Action<Instruction, Message> {
	fn from(value: Task<Message>) -> Self {
		Self::task(value)
	}
}
