use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InitRegistryError {
	#[error("failed to initialize git repository: {0}")]
	GitInit(git2::Error),
	#[error("{0}")]
	WriteFailed(#[from] WriteFailed),
	#[error("{0}")]
	CreateDir(#[from] CreateDirError),
	#[error("failed to commit changes: {0}")]
	CommitFailed(git2::Error),
}

#[derive(Debug, Error)]
pub enum OpenRegistryError {
	#[error("failed to read git repository: {0}")]
	GitOpen(git2::Error),
	#[error("{0}")]
	ReadConfig(#[from] ReadTomlError),
}

#[derive(Debug, Error)]
pub enum AddCrateFromFileError {
	#[error("{0}")]
	InvalidFileName(#[from] InvalidCrateFileName),
	#[error("{0}")]
	ReadFailed(#[from] ReadFailed),
	#[error("{0}")]
	AddCrateError(#[from] AddCrateError),
}

#[derive(Debug, Error)]
pub enum AddCrateError {
	#[error("{0}")]
	ReadIndex(#[from] ReadIndexError),
	#[error("{0}")]
	LockFailed(#[from] LockFailed),
	#[error("{0}")]
	DuplicateIndexEntry(#[from] DuplicateIndexEntry),
	#[error("{0}")]
	WriteFailed(#[from] WriteFailed),
	#[error("failed to commit changes: {0}")]
	CommitFailed(git2::Error),
}

#[derive(Debug, Error)]
pub enum ReadIndexError {
	#[error("{0}")]
	ReadFailed(#[from] ReadFailed),
	#[error("invalid UTF-8 in index: {0}")]
	InvalidUtf8(#[from] InvalidUt8File),
}

impl ReadIndexError {
	/// Check if reading the index failed because the index file does not exist.
	pub fn is_not_found(&self) -> bool {
		if let Self::ReadFailed(e) = self {
			e.error.kind() == std::io::ErrorKind::NotFound
		} else {
			false
		}
	}
}

#[derive(Debug, Error)]
pub enum ReadTomlError {
	#[error("{0}")]
	ReadFailed(#[from] ReadFailed),
	#[error("invalid UTF-8 in index: {0}")]
	ParseToml(#[from] ParseTomlError),
}

#[derive(Debug, Error)]
#[error("failed to parse TOML file: {path}: {error}")]
pub struct ParseTomlError {
	pub path: PathBuf,
	pub error: toml::de::Error,
}

#[derive(Debug, Error)]
#[error("invalid file name for packaged crate: expected $name-$version.crate")]
pub struct InvalidCrateFileName {
	pub path: PathBuf,
}

#[derive(Debug, Error)]
#[error("duplicate index entry: {name}-{version} already exists in registry")]
pub struct DuplicateIndexEntry {
	pub name: String,
	pub version: String,
}

#[derive(Debug, Error)]
#[error("{0}")]
pub enum YankCrateError {
	#[error("failed to commit changes: {0}")]
	CommitFailed(git2::Error),
}

#[derive(Debug, Error)]
#[error("failed to create directory {path}: {error}")]
pub struct CreateDirError {
	/// The path to the directory.
	pub path: PathBuf,

	/// The I/O error that occured.
	pub error: std::io::Error,
}

#[derive(Debug, Error)]
#[error("failed to read from {path}: {error}")]
pub struct ReadFailed {
	/// The file that failed to read.
	pub path: PathBuf,

	/// The I/O error that occured.
	pub error: std::io::Error,
}

#[derive(Debug, Error)]
#[error("failed to read {path}: {error}")]
pub struct InvalidUt8File {
	/// The file that failed to read.
	pub path: PathBuf,

	/// The I/O error that occured.
	pub error: std::str::Utf8Error,
}

#[derive(Debug, Error)]
#[error("failed to write to {path}: {error}")]
pub struct WriteFailed {
	/// The file that failed to read.
	pub path: PathBuf,

	/// The I/O error that occured.
	pub error: std::io::Error,
}

#[derive(Debug, Error)]
#[error("failed to lock {path} for {mode} access: {error}")]
pub struct LockFailed {
	/// The file that failed to read.
	pub path: PathBuf,

	/// The lock mode.
	pub mode: LockMode,

	/// The I/O error that occured.
	pub error: std::io::Error,
}

#[derive(Debug)]
pub enum LockMode {
	Shared,
	Exclusive,
}

impl std::fmt::Display for LockMode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Shared => "shared".fmt(f),
			Self::Exclusive => "exclusive".fmt(f),
		}
	}
}
