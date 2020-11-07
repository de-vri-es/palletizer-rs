use crate::{error, Config};
use std::path::{Path, PathBuf};

pub struct Registry {
	repo: git2::Repository,
}

impl Registry {
	/// Initialize a new registry with a config file.
	pub fn init(path: impl AsRef<Path>, config: &Config) -> Result<Self, error::InitError> {
		let path = path.as_ref();
		let repo = git2::Repository::init(path)
			.map_err(error::InitError::GitInit)?;

		write_new_file(
			path.join("Palletizer.toml"),
			&toml::ser::to_vec(config).unwrap(),
		)?;

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
	///
	/// You must pass the path to a crate as packaged by `cargo package`.
	pub fn add_crate(&mut self, path: impl AsRef<Path>) -> Result<(), error::AddCrateError> {
		let path = path.as_ref();
		let (name, version) = parse_crate_name(path)?;

		// Read the index to find existing versions of the crate.
		let index = match self.read_index(name) {
			Ok(x) => x,
			Err(e) => if e.is_not_found() {
				Vec::new()
			} else {
				return Err(e.into());
			}
		};

		// Check that the version isn't in the index yet.
		if index.iter().find(|x| x == &version).is_some() {
			return Err(error::DuplicateCrateVersion {
				name: name.to_string(),
				version: version.to_string(),
			}.into());
		}

		// TODO: Copy the crate file to the download location,
		// add an entry to the index file.
		// If not, add it and commit.
		todo!();
	}


	/// Yank a crate from the registry.
	pub fn yank_crate(&mut self, name: &str, version: &str) -> Result<(), error::YankCrateError> {
		todo!()
	}

	pub fn read_index(&self, name: &str) -> Result<Vec<String>, error::ReadIndexError> {
		let path = self.index_path(name);
		let data = read_file(&path)?;
		let data = String::from_utf8(data).map_err(|e| error::InvalidUt8File {
			path,
			error: e.utf8_error(),
		})?;
		Ok(data.lines().map(String::from).collect())
	}

	fn index_path(&self, name: &str) -> PathBuf {
		let mut file = match name.as_bytes() {
			&[] => panic!("empty crate names are not supported"),
			&[a] => format!("1/{}/{}", a as char, name),
			&[a, b] => format!("2/{}/{}/{}", a as char, b as char, name),
			&[a, b, c] => format!("3/{}/{}/{}/{}", a as char, b as char, c as char, name),
			&[a, b, c, d, ..] => format!("{}{}/{}{}/{}", a as char, b as char, c as char, d as char, name),
		};
		file.make_ascii_lowercase();

		self.repo.path().join(file)
	}
}

fn write_new_file(path: impl AsRef<Path>, data: &[u8]) -> Result<(), error::WriteFailed> {
	use std::io::Write;
	let path = path.as_ref();

	let map_err = |error| error::WriteFailed {
		error,
		path: path.into(),
	};

	std::fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(path)
		.map_err(map_err)?
		.write_all(&data)
		.map_err(map_err)
}

fn read_file(path: impl AsRef<Path>) -> Result<Vec<u8>, error::ReadFailed> {
	let path = path.as_ref();
	std::fs::read(path)
		.map_err(|error| error::ReadFailed {
			error,
			path: path.into(),
		})
}

fn overwrite_file(path: impl AsRef<Path>, data: &[u8]) -> Result<(), error::WriteFailed> {
	let path = path.as_ref();
	std::fs::write(path, data)
		.map_err(|error| error::WriteFailed {
			error,
			path: path.into(),
		})
}

fn parse_crate_name(path: &Path) -> Result<(&str, &str), error::InvalidCrateFileName> {
	let make_err = || error::InvalidCrateFileName {
		path: path.into(),
	};

	path
		.file_name()
		.ok_or_else(make_err)?
		.to_str()
		.ok_or_else(make_err)?
		.strip_suffix(".crate")
		.ok_or_else(make_err)?
		.rpartition('-')
		.ok_or_else(make_err)
}

trait Rpartition {
	fn rpartition(&self, split: char) -> Option<(&Self, &Self)>;
}

impl Rpartition for str {
	fn rpartition(&self, split: char) -> Option<(&str, &str)> {
		let mut parts = self.rsplitn(2, split);
		Some((parts.next().unwrap(), parts.next()?))
	}
}
