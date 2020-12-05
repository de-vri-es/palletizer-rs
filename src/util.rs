#![allow(unused)]

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::error::Error;

/// Get the HEAD of a git repository, if it exists.
///
/// This returns None if the HEAD is an unborn branch without any commits.
fn get_head(repo: &git2::Repository) -> Result<Option<git2::Commit>, Error> {
	let head = match repo.head() {
		Ok(head) => head,
		Err(e) => if e.code() == git2::ErrorCode::UnbornBranch {
			return Ok(None);
		} else {
			return Err(Error::new(format!("failed to determine repository HEAD: {}", e)));
		}
	};

	let head = head.peel_to_commit()
		.map_err(|e| Error::new(format!("failed to resolve HEAD to commit hash: {}", e)))?;

	Ok(Some(head))
}

/// Add the given files to the index and commit the index.
pub fn add_commit(repo: &git2::Repository, message: &str, files: &[impl AsRef<Path>]) -> Result<git2::Oid, Error> {
	let signature = repo.signature()
		.map_err(|e| Error::new(format!("failed to determine author for git commit: {}", e)))?;

	let head = get_head(repo)?;

	let mut index = repo.index()
		.map_err(|e| Error::new(format!("failed to get index of repository: {}", e)))?;

	// Make sure the repo isn't busy rebasing or anything like that.
	if repo.state() != git2::RepositoryState::Clean {
		return Err(Error::new(format!("repository is in {:?} state", repo.state())));
	}

	// Make sure the index is clean (don't care about the work tree).
	if let Some(head) = &head {
		let head_tree = head.tree()
				.map_err(|e| Error::new(format!("failed to find tree for HEAD: {}", e)))?;
		let staged = repo.diff_tree_to_index(Some(&head_tree), Some(&index), None)
			.map_err(|e| Error::new(format!("failed to compare tree with index: {}", e)))?;
		if staged.deltas().next().is_some() {
			return Err(Error::new(format!("index already contains staged changes")));
		}
	} else if !index.is_empty() {
		return Err(Error::new(format!("index already contains staged changes")));
	}

	// Add the files to the index.
	for path in files {
		let path = path.as_ref();
		index.add_path(path)
			.map_err(|e| Error::new(format!("failed to add {} to index: {}", path.display(), e)))?;
	}
	index.write().map_err(|e| Error::new(format!("failed to write index back to disk: {}", e)))?;

	// Create a tree from the index.
	let tree = index.write_tree()
		.map_err(|e| Error::new(format!("failed to write index to a tree: {}", e)))?;
	let tree = repo.find_tree(tree)
		.map_err(|e| Error::new(format!("failed to find newly written tree with OID {}: {}", tree, e)))?;

	// Create the commit.
	let result = if let Some(head) = head {
		repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[&head])
	} else {
		repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])
	};
	result.map_err(|e| Error::new(format!("failed to create commit: {}", e)))
}

/// Create a directory and all leading directories.
pub fn create_dirs(path: impl AsRef<Path>) -> Result<(), Error> {
	let path = path.as_ref();
	std::fs::create_dir_all(path)
		.map_err(|e| Error::new(format!("failed to create directory {}: {}", path.display(), e)))
}

/// Open a file for reading, locked for shared access.
pub fn open_file_read(path: impl AsRef<Path>) -> Result<File, Error> {
	let path = path.as_ref();
	let file = std::fs::OpenOptions::new()
		.read(true)
		.open(path)
		.map_err(|e| Error::new(format!("failed to open {} for reading: {}", path.display(), e)))?;
	lock_shared(&file, path)?;
	Ok(file)
}

/// Open a file for reading and writing, locked for exclusive access.
pub fn open_file_read_write(path: impl AsRef<Path>) -> Result<File, Error> {
	let path = path.as_ref();
	let file = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.open(path)
		.map_err(|e| Error::new(format!("failed to open {} for reading and writing: {}", path.display(), e)))?;
	lock_exclusive(&file, path)?;
	Ok(file)
}

/// Open a file for writing, truncating it and locked for exclusive access.
///
/// The file and all parent directories are created if they do not yet exist.
pub fn open_file_overwrite(path: impl AsRef<Path>) -> Result<File, Error> {
	let path = path.as_ref();

	if let Some(parent) = path.parent() {
		create_dirs(parent)?;
	}

	let file = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(true)
		.open(path)
		.map_err(|e| Error::new(format!("failed to open {} for writing: {}", path.display(), e)))?;
	lock_exclusive(&file, path)?;
	Ok(file)
}

/// Open a file for appending, locked for exclusive access.
///
/// The file and all parent directories are created if they do not yet exist.
pub fn open_file_append(path: impl AsRef<Path>) -> Result<File, Error> {
	let path = path.as_ref();

	if let Some(parent) = path.parent() {
		create_dirs(parent)?;
	}

	let file = std::fs::OpenOptions::new()
		.write(true)
		.append(true)
		.create(true)
		.open(path)
		.map_err(|e| Error::new(format!("failed to open {} for appending: {}", path.display(), e)))?;
	lock_exclusive(&file, path)?;
	Ok(file)
}

/// Create a new file, opened for writing and locked for exclusive access.
///
/// This fails if the file already exists.
///
/// All parent folders are created as needed.
pub fn create_new_file(path: impl AsRef<Path>) -> Result<File, Error> {
	let path = path.as_ref();

	if let Some(parent) = path.parent() {
		create_dirs(parent)?;
	}

	let file = std::fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(path)
		.map_err(|e| Error::new(format!("failed to create {}: {}", path.display(), e)))?;
	lock_exclusive(&file, path)?;
	Ok(file)
}

/// Create a new file with the given contents.
///
/// This fails if the file already exists.
pub fn write_new_file(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<(), Error> {
	let path = path.as_ref();
	create_new_file(path)?
		.write_all(data.as_ref())
		.map_err(|e| Error::new(format!("failed to write to {}: {}", path.display(), e)))
}

/// Write to a file, overwriting the contents if it exists already.
pub fn overwrite_file(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<(), Error> {
	let path = path.as_ref();
	open_file_overwrite(path)?
		.write_all(data.as_ref())
		.map_err(|e| Error::new(format!("failed to write to {}: {}", path.display(), e)))
}

/// Truncate a file to zero length.
///
/// In addition to truncating the file, the file pointer is reset to the start of the file.
///
/// No locks are taken. The file should already be locked if desired.
pub fn truncate_file(file: &mut File, path: impl AsRef<Path>) -> Result<(), Error> {
	use std::io::Seek;
	let path = path.as_ref();
	file.seek(std::io::SeekFrom::Start(0))
		.map_err(|e| Error::new(format!("failed to seek to file start of {}: {}", path.display(), e)))?;
	file.set_len(0)
		.map_err(|e| Error::new(format!("failed to truncate {}: {}", path.display(), e)))?;
	Ok(())
}

/// Overwrite the contents of an open file.
///
/// No locks are taken. The file should already be locked if desired.
pub fn overwrite_contents(file: &mut File, path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<(), Error> {
	let path = path.as_ref();
	truncate_file(file, path)?;
	file.write_all(data.as_ref())
		.map_err(|e| Error::new(format!("failed to write to {}: {}", path.display(), e)))?;
	Ok(())
}

/// Read the contents of a file.
pub fn read_file(path: impl AsRef<Path>) -> Result<Vec<u8>, Error> {
	use std::io::Read;
	let path = path.as_ref();
	let mut buffer = Vec::new();
	open_file_read(path)?
		.read_to_end(&mut buffer)
		.map_err(|e| Error::new(format!("failed to read from {}: {}", path.display(), e)))?;
	Ok(buffer)
}

/// Lock a file for exclusive access.
pub fn lock_exclusive(file: &impl fs2::FileExt, path: impl AsRef<Path>) -> Result<(), Error> {
	let path = path.as_ref();
	file.lock_exclusive()
		.map_err(|e| Error::new(format!("failed to lock {} for exclusive access: {}", path.display(), e)))
}

/// Lock a file for shared access.
pub fn lock_shared(file: &impl fs2::FileExt, path: impl AsRef<Path>) -> Result<(), Error> {
	let path = path.as_ref();
	file.lock_shared()
		.map_err(|e| Error::new(format!("failed to lock {} for shared access: {}", path.display(), e)))
}

/// Read a file containing TOML.
pub fn read_toml<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Error> {
	let path = path.as_ref();
	let data = read_file(path)?;
	let parsed = parse_toml(&data, &path.display())?;
	Ok(parsed)
}

/// Parse bytes as a TOML structure.
pub fn parse_toml<'a, T: serde::Deserialize<'a>>(data: &'a [u8], path: &impl std::fmt::Display) -> Result<T, Error> {
	toml::from_slice(&data)
		.map_err(|e| Error::new(format!("failed to parse TOML from {}: {}", path, e)))
}

/// Compute the sha256sum of some data and return it as lowercase hex string.
pub fn compute_sha256_hex(data: impl AsRef<[u8]>) -> String {
	use sha2::{Digest, Sha256};
	format!("{:x}", Sha256::digest(data.as_ref()))
}
