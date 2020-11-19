use std::io::Write;
use std::path::Path;

use crate::error::Error;

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

pub fn compute_sha256_hex(data: impl AsRef<[u8]>) -> String {
	use sha2::{Digest, Sha256};
	format!("{:x}", Sha256::digest(data.as_ref()))
}
