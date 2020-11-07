use crate::{error, Config};
use std::path::Path;

pub struct Registry {
	repo: git2::Repository,
}

impl Registry {
	/// Initialize a new registry with a config file.
	pub fn init(path: impl AsRef<Path>, config: &Config) -> Result<Self, error::InitError> {
		let path = path.as_ref();
		let repo = git2::Repository::init(path)
			.map_err(error::InitError::GitInit)?;

		write_new(
			path.join("Palletizer.toml"),
			&toml::ser::to_vec(config).unwrap(),
		).map_err(error::InitError::WriteConfig)?;

		Ok(Self {
			repo
		})
	}

	/// Open an existing registry.
	pub fn open(path: impl AsRef<Path>) -> Result<Self, error::OpenError> {
		let repo = git2::Repository::open(path)
			.map_err(error::OpenError::GitOpen)?;
		Ok(Self { repo })
	}

	/// Get the path of the registry.
	pub fn path(&self) -> &Path {
		self.repo.path()
	}

	/// Add a crate to the registry.
	pub fn add_crate(&mut self, path: impl AsRef<Path>) -> Result<Self, error::AddCrateError> {
		todo!();
	}

	/// Yank a crate from the registry.
	pub fn yank_crate(&mut self, name: &str, version: &str) -> Result<Self, error::YankCrateError> {
		todo!()
	}
}

fn write_new(path: impl AsRef<Path>, data: &[u8]) -> std::io::Result<()> {
	use std::io::Write;

	std::fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(path)?
		.write_all(&data)
}
