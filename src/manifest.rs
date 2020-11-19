use libflate::gzip;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use crate::error::Error;

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
	pub package: Package,
	pub links: Option<String>,
	#[serde(default)]
	pub features: BTreeMap<String, Vec<String>>,
	#[serde(default)]
	pub dependencies: BTreeMap<String, Dependency>,
	#[serde(default)]
	pub dev_dependencies: BTreeMap<String, Dependency>,
	#[serde(default)]
	pub build_dependencies: BTreeMap<String, Dependency>,
	#[serde(default)]
	pub target: BTreeMap<String, Dependencies>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Dependencies {
	#[serde(default)]
	pub dependencies: BTreeMap<String, Dependency>,
	#[serde(default)]
	pub dev_dependencies: BTreeMap<String, Dependency>,
	#[serde(default)]
	pub build_dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Package {
	pub name: String,
	pub version: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Dependency {
	pub version: String,
	#[serde(default)]
	pub optional: bool,
	#[serde(default)]
	pub features: Vec<String>,
	#[serde(default = "default_true")]
	pub default_features: bool,
	pub package: Option<String>,
	pub registry: Option<String>,
}

fn default_true() -> bool { true }

fn extract_file<P: AsRef<Path>, R: Read>(path: P, archive: R) -> Result<Option<Vec<u8>>, Error> {
	let path = path.as_ref();
	let mut archive = archive;
	let archive = gzip::Decoder::new(&mut archive)
		.map_err(|e| Error::new(format!("failed to initialize gzip decoder: {}", e)))?;
	let mut archive = tar::Archive::new(archive);

	let entries = archive.entries()
		.map_err(|e| Error::new(format!("failed to read archive header: {}", e)))?;
	for file in entries {
		let mut file = file.map_err(|e| Error::new(format!("failed to read archive entry header: {}", e)))?;
		let entry_path = file.path()
			.map_err(|e| Error::new(format!("acrhive entry contains non-UTF8 path: {}", e)))?;

		if entry_path == path {
			let mut data = Vec::new();
			file.read_to_end(&mut data)
				.map_err(|e| Error::new(format!("failed to read archive data for {}: {}", path.display(), e)))?;
			return Ok(Some(data))
		}
	}

	Ok(None)
}

pub fn extract<R: Read>(name: &str, version: &str, archive: R) -> Result<Manifest, Error> {
	let manifest_path = format!("{}-{}/Cargo.toml", name, version);
	let data = extract_file(&manifest_path, archive)?
		.ok_or_else(|| Error::new(format!("failed to find {} in package archive", manifest_path)))?;
	Ok(toml::from_slice(&data).unwrap())
}
