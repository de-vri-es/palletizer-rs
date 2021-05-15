use libflate::gzip;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;

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

pub fn extract<R: Read>(archive: R) -> Result<Manifest, Error> {
	let mut archive = archive;
	let archive = gzip::Decoder::new(&mut archive)
		.map_err(|e| Error::new(format!("failed to initialize gzip decoder: {}", e)))?;
	let mut archive = tar::Archive::new(archive);

	let entries = archive.entries()
		.map_err(|e| Error::new(format!("failed to read archive header: {}", e)))?;
	for file in entries {
		let mut file = file.map_err(|e| Error::new(format!("failed to read archive entry header: {}", e)))?;
		let entry_path = file.path()
			.map_err(|e| Error::new(format!("acrhive entry contains non-UTF8 path: {}", e)))?
			.to_path_buf();

		if entry_path.components().count() == 2 && entry_path.ends_with("Cargo.toml") {
			let mut data = Vec::new();
			file.read_to_end(&mut data)
				.map_err(|e| Error::new(format!("failed to read archive data for {}: {}", entry_path.display(), e)))?;
			let manifest = toml::from_slice(&data)
				.map_err(|e| Error::new(format!("failed to parse manifest from archive: {}: {}", entry_path.display(), e)))?;
			return Ok(manifest)
		}
	}

	Err(Error::new("failed to find manifest in archive".into()))
}
