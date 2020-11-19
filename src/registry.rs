use crate::{error, util, Config};
use std::path::{Path, PathBuf};

pub struct Registry {
	config: Config,
	repo: git2::Repository,
}

impl Registry {
	/// Initialize a new registry with a config file.
	pub fn init(path: impl AsRef<Path>, config: Config) -> Result<Self, error::InitRegistryError> {
		let path = path.as_ref();
		let repo = git2::Repository::init(path)
			.map_err(error::InitRegistryError::GitInit)?;

		// Keep track of files to commit.
		let mut created_files: Vec<PathBuf> = Vec::new();

		// Write Palletizer config file.
		util::write_new_file(
			path.join("Palletizer.toml"),
			&toml::ser::to_vec(&config).unwrap(),
		)?;
		created_files.push("Palletizer.toml".into());

		// Add crate directory to .gitignore if it is a subdir of the repository.
		if let Ok(rel_crate_dir) = path.join(&config.crate_dir).strip_prefix(path) {
			util::write_new_file(path.join(".gitignore"), &format!("{}\n", rel_crate_dir.display()))?;
			created_files.push(rel_crate_dir.into());
		}

		// Create webroot with `config.json`.
		let webroot = path.join("webroot");
		util::create_dirs(&webroot)?;
		util::write_new_file(webroot.join("config.json"), config.cargo_json().as_bytes())?;
		created_files.push("webroot/config.json".into());

		// Commit the created files.
		util::add_commit(&repo, "Initialize empty registry.", &created_files)
			.map_err(error::InitRegistryError::CommitFailed)?;

		Ok(Self { config, repo })
	}

	/// Open an existing registry.
	pub fn open(path: impl AsRef<Path>) -> Result<Self, error::OpenRegistryError> {
		let path = path.as_ref();
		let repo = git2::Repository::open(path)
			.map_err(error::OpenRegistryError::GitOpen)?;
		let config: Config = util::read_toml(path.join("Palletizer.toml"))?;
		Ok(Self { config, repo })
	}

	/// Get the path of the registry.
	pub fn path(&self) -> &Path {
		self.repo.workdir().unwrap()
	}

	/// Add a crate to the registry.
	///
	/// You must pass the path to a crate as packaged by `cargo package`.
	pub fn add_crate(&mut self, name: &str, version: &str, data: &[u8]) -> Result<(), error::AddCrateError> {
		use std::io::Write;

		let index_path_rel = self.index_path_rel(name);
		let index_path = self.path().join(&index_path_rel);
		util::create_dirs(index_path.parent().unwrap())?;
		let mut index_file = std::fs::OpenOptions::new()
			.read(true)
			.append(true)
			.create(true)
			.open(&index_path)
			.map_err(|error| error::ReadIndexError::ReadFailed(error::ReadFailed {
				path: index_path.clone(),
				error,
			}))?;

		util::lock_exclusive(&index_file, &index_path)?;

		// Check that the version isn't in the index yet.
		let index = read_index(&mut index_file, &index_path)?;
		if index.iter().find(|x| x == &version).is_some() {
			return Err(error::DuplicateIndexEntry {
				name: name.to_string(),
				version: version.to_string(),
			}.into());
		}

		// Write the crate file.
		util::write_new_file(self.crate_path_abs(name, version), data)?;

		// Add the index entry.
		writeln!(&mut index_file, "{}", version).map_err(|error| error::WriteFailed {
			path: index_path.into(),
			error,
		})?;

		// Commit the changes.
		util::add_commit(&self.repo, &format!("Add {}-{}", name, version), &[index_path_rel])
			.map_err(error::AddCrateError::CommitFailed)?;

		Ok(())
	}

	/// Add a crate to the registry.
	///
	/// You must pass the path to a crate as packaged by `cargo package`.
	pub fn add_crate_from_file(&mut self, path: impl AsRef<Path>) -> Result<(), error::AddCrateFromFileError> {
		let path = path.as_ref();
		let (name, version) = parse_crate_name(path)?;
		let data = util::read_file(path)?;
		self.add_crate(name, version, &data)?;
		Ok(())
	}

	/// Yank a crate from the registry.
	pub fn yank_crate(&mut self, name: &str, version: &str) -> Result<(), error::YankCrateError> {
		todo!()
	}

	fn index_path_rel(&self, name: &str) -> PathBuf {
		let mut file = match name.as_bytes() {
			&[] => panic!("empty crate names are not supported"),
			&[a] => format!("1/{}/{}", a as char, name),
			&[a, b] => format!("2/{}/{}/{}", a as char, b as char, name),
			&[a, b, c] => format!("3/{}/{}/{}/{}", a as char, b as char, c as char, name),
			&[a, b, c, d, ..] => format!("{}{}/{}{}/{}", a as char, b as char, c as char, d as char, name),
		};
		file.make_ascii_lowercase();

		file.into()
	}

	fn crate_path(&self, name: &str, version: &str) -> PathBuf {
		self.config.crate_dir.join(format!("{name}/{name}-{version}.crate", name = name, version = version))
	}

	fn crate_path_abs(&self, name: &str, version: &str) -> PathBuf {
		self.path().join(&self.crate_path(name, version))
	}
}

pub fn read_index<R: std::io::Read>(mut stream: R, path: &Path) -> Result<Vec<String>, error::ReadIndexError> {
	let mut data = Vec::new();
	stream.read_to_end(&mut data).map_err(|error| error::ReadFailed {
		path: path.into(),
		error,
	})?;

	let data = String::from_utf8(data).map_err(|error| error::InvalidUt8File {
		path: path.into(),
		error: error.utf8_error(),
	})?;

	Ok(data.lines().map(String::from).collect())
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
