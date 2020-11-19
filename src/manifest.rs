use libflate::gzip;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

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
	pub target: Option<toml::value::Table>,
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
	pub features: Vec<String>,
	#[serde(default)]
	pub optional: bool,
	pub registry: Option<String>,
	pub package: Option<String>,
}

fn extract_file<P: AsRef<Path>, R: Read>(path: P, archive: R) -> std::io::Result<Option<Vec<u8>>> {
	let path = path.as_ref();
	let mut archive = archive;
	let archive = gzip::Decoder::new(&mut archive)?;
	let mut archive = tar::Archive::new(archive);

	let entries = archive.entries()?;
	for file in entries {
		let mut file = file?;
		if file.path()? == path {
			let mut data = Vec::new();
			file.read_to_end(&mut data)?;
			return Ok(Some(data))
		}
	}

	Ok(None)
}

pub fn extract<R: Read>(name: &str, version: &str, archive: R) -> std::io::Result<Manifest> {
	use std::io::Write;
	let manifest_path = format!("{}-{}/Cargo.toml", name, version);
	let data = extract_file(&manifest_path, archive)?
		.ok_or_else(|| std::io::Error::new(
				std::io::ErrorKind::NotFound,
				format!("failed to find {} in package archive", manifest_path)
		))?;
	std::io::stdout().write_all(&data).unwrap();
	println!();
	Ok(toml::from_slice(&data).unwrap())
}
