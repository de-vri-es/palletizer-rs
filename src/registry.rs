use crate::{index, manifest, util, Config};
use crate::error::Error;

use std::path::{Path, PathBuf};

pub struct Registry {
	path: PathBuf,
	config: Config,
	repo: git2::Repository,
}

// I think read-only access from multiple threads is fine.
//
// Without this, an Arc<RwLock<Registry>> would not be Send,
// meaning we would always need to use a mutex.
// It would be a waste to unnecessarily serialize read-only access to the index repo.
unsafe impl Sync for Registry {}

impl Registry {
	/// Initialize a new registry with a config file.
	pub fn init(path: impl AsRef<Path>, config: Config) -> Result<Self, Error> {
		let path = path.as_ref().to_path_buf();

		// Write Palletizer config file.
		util::write_new_file(
			path.join("Palletizer.toml"),
			&toml::ser::to_vec(&config).unwrap(),
		)?;

		// Create the index repository.
		let index_path = path.join(&config.index_dir);
		util::create_dirs(&index_path)?;
		let repo = git2::Repository::init(&index_path)
			.map_err(|e| Error::new(format!("failed to initialize git repository at {}: {}", path.display(), e)))?;

		// Add `config.json`.
		util::write_new_file(index_path.join("config.json"), config.cargo_json().as_bytes())?;

		// Commit the created files.
		util::add_commit(&repo, "Initialize empty registry index.", &["config.json"])?;

		Ok(Self { path, config, repo })
	}

	/// Open an existing registry.
	pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
		let path = path.as_ref().to_path_buf();
		let config: Config = util::read_toml(path.join("Palletizer.toml"))?;

		let index_path = path.join(&config.index_dir);

		let repo = git2::Repository::open(&index_path)
			.map_err(|e| Error::new(format!("failed to open git repository at {}: {}", index_path.display(), e)))?;
		Ok(Self { path, config, repo })
	}

	/// Get the path of the registry.
	pub fn path(&self) -> &Path {
		&self.path
	}

	/// Get the path of the index repository.
	pub fn index_dir(&self) -> PathBuf {
		self.path.join(&self.config.index_dir)
	}

	/// Get the path of the crate directory.
	pub fn crate_dir(&self) -> PathBuf {
		self.path.join(&self.config.crate_dir)
	}

	/// Add a crate to the registry using the supplied metadata.
	pub fn add_crate_with_metadata(&mut self, metadata: &index::Entry, data: &[u8]) -> Result<(), Error> {
		use std::io::Write;

		let metadata_json = serde_json::to_string(&metadata)
			.map_err(|e| Error::new(format!("failed to serialize index metadata: {}", e)))?;

		let index_path_rel = self.index_path_rel(&metadata.name);
		let index_path_abs = self.index_dir().join(&index_path_rel);
		util::create_dirs(index_path_abs.parent().unwrap())?;
		let mut index_file = std::fs::OpenOptions::new()
			.read(true)
			.append(true)
			.create(true)
			.open(&index_path_abs)
			.map_err(|e| Error::new(format!("failed to open {} for writing: {}", index_path_abs.display(), e)))?;

		util::lock_exclusive(&index_file, &index_path_abs)?;

		// Check that the version isn't in the index yet.
		let index = read_index(&mut index_file, &index_path_abs)?;
		if index.iter().any(|x| x.version == metadata.version) {
			return Err(Error::new(format!("duplicate crate: {}-{} already exists in the index", metadata.name, metadata.version)));
		}

		// Write the crate file.
		util::write_new_file(self.crate_path_abs(&metadata.name, &metadata.version), data)?;

		// Add the index entry.
		writeln!(&mut index_file, "{}", &metadata_json)
			.map_err(|e| Error::new(format!("failed to write to index file {}: {}", index_path_abs.display(), e)))?;

		// Commit the changes.
		util::add_commit(&self.repo, &format!("Add {}-{}", metadata.name, metadata.version), &[index_path_rel])
			.map_err(|e| Error::new(format!("failed to commit changes: {}", e)))?;

		Ok(())
	}

	/// Add a crate to the registry.
	///
	/// You must pass the path to a crate as packaged by `cargo package`.
	pub fn add_crate(&mut self, data: &[u8]) -> Result<(), Error> {
		// Extract the manifest.
		let manifest = manifest::extract(data)?;
		let sha256_hexsum = util::compute_sha256_hex(data);
		let metadata = index::Entry::from_manifest(manifest, sha256_hexsum)?;

		self.add_crate_with_metadata(&metadata, data)
	}

	/// Add a crate to the registry.
	///
	/// You must pass the path to a crate as packaged by `cargo package`.
	pub fn add_crate_from_file(&mut self, path: impl AsRef<Path>) -> Result<(), Error> {
		let data = util::read_file(path.as_ref())?;
		self.add_crate(&data)
	}

	/// Yank a crate from the registry.
	///
	/// Returns true if the crate was yanked,
	/// and false if the crate was already yanked.
	///
	/// If the crate is not found or if an other error occures,
	/// an error is returned.
	pub fn yank_crate(&mut self, name: &str, version: &str) -> Result<bool, Error> {
		let index_path_rel = self.index_path_rel(name);
		let index_path_abs = self.index_dir().join(&index_path_rel);
		let mut index_file = util::open_file_read_write(&index_path_abs)?;
		let mut index = index::read_index(&mut index_file)?;

		let mut found = 0;
		let mut yanked = 0;
		for entry in &mut index {
			if entry.version == version {
				found += 1;
				if !entry.yanked {
					entry.yanked = true;
					yanked += 1;
				}
			}
		}

		if found == 0 {
			return Err(Error::new(format!("failed to yank {}-{}: no such crate in index", name ,version)));
		}

		if yanked > 0 {
			util::truncate_file(&mut index_file, &index_path_abs)?;
			index::write_index(&mut index_file, &index_path_abs, &index)?;

			// Commit the changes.
			util::add_commit(&self.repo, &format!("Yanked {}-{}", name, version), &[index_path_rel])
				.map_err(|e| Error::new(format!("failed to commit changes: {}", e)))?;
			Ok(true)
		} else{
			Ok(false)
		}
	}

	/// Unyank a crate from the registry.
	///
	/// Returns true if the crate was unyanked,
	/// and false if the crate was already unyanked.
	///
	/// If the crate is not found or if an other error occures,
	/// an error is returned.
	pub fn unyank_crate(&mut self, name: &str, version: &str) -> Result<bool, Error> {
		let index_path_rel = self.index_path_rel(name);
		let index_path_abs = self.index_dir().join(&index_path_rel);
		let mut index_file = util::open_file_read_write(&index_path_abs)?;
		let mut index = index::read_index(&mut index_file)?;

		let mut found = 0;
		let mut unyanked = 0;
		for entry in &mut index {
			if entry.version == version {
				found += 1;
				if entry.yanked {
					entry.yanked = false;
					unyanked += 1;
				}
			}
		}

		if found == 0 {
			return Err(Error::new(format!("failed to unyank {}-{}: no such crate in index", name ,version)));
		}

		if unyanked > 0 {
			util::truncate_file(&mut index_file, &index_path_abs)?;
			index::write_index(&mut index_file, &index_path_abs, &index)?;

			// Commit the changes.
			util::add_commit(&self.repo, &format!("Yanked {}-{}", name, version), &[index_path_rel])
				.map_err(|e| Error::new(format!("failed to commit changes: {}", e)))?;
			Ok(true)
		} else{
			Ok(false)
		}

	}

	#[allow(clippy::match_ref_pats)]
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

	fn crate_path_rel(&self, name: &str, version: &str) -> PathBuf {
		self.config.crate_dir.join(format!("{name}/{name}-{version}.crate", name = name, version = version))
	}

	fn crate_path_abs(&self, name: &str, version: &str) -> PathBuf {
		self.path().join(&self.crate_path_rel(name, version))
	}
}

pub fn read_index<R: std::io::Read>(mut stream: R, path: &Path) -> Result<Vec<index::Entry>, Error> {
	let mut data = Vec::new();
	stream.read_to_end(&mut data).map_err(|e| Error::new(format!("failed to read from {}: {}", path.display(), e)))?;

	data.split(|&c| c == b'\n')
		.enumerate()
		.filter(|(_i, line)| !line.is_empty())
		.map(|(i, line)| {
			serde_json::from_slice(line)
				.map_err(|e| Error::new(format!("failed to parse index entry at {}:{}: {}", path.display(), i, e)))
		})
		.collect()
}

trait Rpartition {
	fn rpartition(&self, split: char) -> Option<(&Self, &Self)>;
}

impl Rpartition for str {
	fn rpartition(&self, split: char) -> Option<(&str, &str)> {
		let mut parts = self.rsplitn(2, split);
		let right = parts.next().unwrap();
		let left = parts.next()?;
		Some((left, right))
	}
}
