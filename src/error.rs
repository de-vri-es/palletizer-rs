#[derive(Debug)]
pub struct Error {
	message: String,
}

impl Error {
	pub(crate) fn new(message: String) -> Self {
		Self { message }
	}
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.message.fmt(f)
	}
}
