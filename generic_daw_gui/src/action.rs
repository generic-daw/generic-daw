use iced::Task;

pub struct Action<Instruction, Message> {
	pub instruction: Option<Instruction>,
	pub task: Task<Message>,
}

impl<Instruction, Message> Default for Action<Instruction, Message> {
	fn default() -> Self {
		Self {
			instruction: None,
			task: Task::none(),
		}
	}
}

impl<Instruction, Message> Action<Instruction, Message> {
	pub fn none() -> Self {
		Self::default()
	}

	pub fn instruction(instruction: Instruction) -> Self {
		Self::none().with_instruction(instruction)
	}

	pub fn with_instruction(mut self, instruction: Instruction) -> Self {
		self.instruction = Some(instruction);
		self
	}

	pub fn task(task: Task<Message>) -> Self {
		Self::none().with_task(task)
	}

	pub fn with_task(mut self, task: Task<Message>) -> Self {
		self.task = task;
		self
	}
}

impl<Instruction, Message> From<Task<Message>> for Action<Instruction, Message> {
	fn from(value: Task<Message>) -> Self {
		Self::task(value)
	}
}
