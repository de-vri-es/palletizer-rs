use std::io::Write;
use std::path::Path;

use crate::error::Error;

/// Add the given files to the index and commit the index.
pub fn add_commit(repo: &git2::Repository, message: &str, files: &[impl AsRef<Path>]) -> Result<git2::Oid, Error> {
	let signature = repo.signature()
		.map_err(|e| Error::new(format!("failed to determine author for git commit: {}", e)))?;
	let head = repo.head()
		.map_err(|e| Error::new(format!("failed to determine repository HEAD: {}", e)))?
		.peel_to_commit()
		.map_err(|e| Error::new(format!("failed to resolve HEAD to commit hash: {}", e)))?;


	let mut index = repo.index()
		.map_err(|e| Error::new(format!("failed to retrieve repository index: {}", e)))?;
	for path in files {
		let path = path.as_ref();
		index.add_path(path)
			.map_err(|e| Error::new(format!("failed to add {} to index: {}", path.display(), e)))?;
	}

	let tree = index.write_tree()
		.map_err(|e| Error::new(format!("failed to write index to a tree: {}", e)))?;
	let tree = repo.find_tree(tree)
		.map_err(|e| Error::new(format!("failed to find newly written tree with OID {}: {}", tree, e)))?;

	repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[&head])
		.map_err(|e| Error::new(format!("failed to create commit: {}", e)))
}

/// Create a directory and all leading directories.
pub fn create_dirs(path: impl AsRef<Path>) -> Result<(), Error> {
	let path = path.as_ref();
	std::fs::create_dir_all(path)
		.map_err(|e| Error::new(format!("failed to create directory {}: {}", path.display(), e)))
}

/// Create a new file with the given contents.
///
/// This fails if the file already exists.
pub fn write_new_file(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<(), Error> {
	let path = path.as_ref();

	std::fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(path)
		.map_err(|e| Error::new(format!("failed to create new file {} for writing: {}", path.display(), e)))?
		.write_all(data.as_ref())
		.map_err(|e| Error::new(format!("failed to write to {}: {}", path.display(), e)))
}

/// Write to a file, overwriting the contents if it exists already.
pub fn overwrite_file(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<(), Error> {
	let path = path.as_ref();
	std::fs::write(path, data.as_ref())
		.map_err(|e| Error::new(format!("failed to overwrite {}: {}", path.display(), e)))
}

/// Read the contents of a file.
pub fn read_file(path: impl AsRef<Path>) -> Result<Vec<u8>, Error> {
	let path = path.as_ref();
	std::fs::read(path)
		.map_err(|e| Error::new(format!("failed to read from {}: {}", path.display(), e)))
}

pub fn lock_exclusive(file: &impl fs2::FileExt, path: impl AsRef<Path>) -> Result<(), Error> {
	let path = path.as_ref();
	file.lock_exclusive()
		.map_err(|e| Error::new(format!("failed to lock {} for exclusive access: {}", path.display(), e)))
}

pub fn lock_shared(file: &impl fs2::FileExt, path: impl AsRef<Path>) -> Result<(), Error> {
	let path = path.as_ref();
	file.lock_shared()
		.map_err(|e| Error::new(format!("failed to lock {} for shared access: {}", path.display(), e)))
}

pub fn read_toml<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Error> {
	let path = path.as_ref();
	let data = read_file(path)?;
	let parsed = parse_toml(&data, &path.display())?;
	Ok(parsed)
}

pub fn parse_toml<'a, T: serde::Deserialize<'a>>(data: &'a [u8], path: &impl std::fmt::Display) -> Result<T, Error> {
	toml::from_slice(&data)
		.map_err(|e| Error::new(format!("failed to parse TOML from {}: {}", path, e)))
}
