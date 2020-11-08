use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error;

/// Add the given files to the index and commit the index.
pub fn add_commit(repo: &git2::Repository, message: &str, files: &[impl AsRef<Path>]) -> Result<git2::Oid, git2::Error> {
	let signature = repo.signature()?;
	let head = repo.head()?.peel_to_commit()?;

	let mut index = repo.index()?;
	for path in files {
		index.add_path(path.as_ref())?;
	}

	let tree = index.write_tree()?;
	let tree = repo.find_tree(tree)?;

	repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[&head])
}

/// Create a directory and all leading directories.
pub fn create_dirs(path: impl AsRef<Path>) -> Result<(), error::CreateDirError> {
	let path = path.as_ref();
	std::fs::create_dir_all(path)
		.map_err(|error| error::CreateDirError {
			path: path.into(),
			error,
		})
}

/// Create a new file with the given contents.
///
/// This fails if the file already exists.
pub fn write_new_file(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<(), error::WriteFailed> {
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
		.write_all(data.as_ref())
		.map_err(map_err)
}

/// Write to a file, overwriting the contents if it exists already.
pub fn overwrite_file(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<(), error::WriteFailed> {
	let path = path.as_ref();
	std::fs::write(path, data.as_ref())
		.map_err(|error| error::WriteFailed {
			error,
			path: path.into(),
		})
}

/// Read the contents of a file.
pub fn read_file(path: impl AsRef<Path>) -> Result<Vec<u8>, error::ReadFailed> {
	let path = path.as_ref();
	std::fs::read(path)
		.map_err(|error| error::ReadFailed {
			error,
			path: path.into(),
		})
}

pub fn lock_exclusive(file: &impl fs2::FileExt, path: impl Into<PathBuf>) -> Result<(), error::LockFailed> {
	file.lock_exclusive().map_err(|error| error::LockFailed {
		path: path.into(),
		mode: error::LockMode::Exclusive,
		error,
	})
}

pub fn lock_shared(file: &impl fs2::FileExt, path: impl Into<PathBuf>) -> Result<(), error::LockFailed> {
	file.lock_exclusive().map_err(|error| error::LockFailed {
		path: path.into(),
		mode: error::LockMode::Exclusive,
		error,
	})
}

pub fn read_toml<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, error::ReadTomlError> {
	let path = path.as_ref();
	let data = read_file(path)?;
	let parsed = parse_toml(&data, path)?;
	Ok(parsed)
}

pub fn parse_toml<'a, T: serde::Deserialize<'a>>(data: &'a [u8], path: impl Into<PathBuf>) -> Result<T, error::ParseTomlError> {
	toml::from_slice(&data) .map_err(|error| error::ParseTomlError {
		path: path.into(),
		error,
	})
}
