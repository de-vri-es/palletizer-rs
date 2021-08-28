use std::collections::BTreeMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::error::Error;
use crate::manifest::{Manifest, Dependency as ManifestDependency};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
	pub name: String,
	#[serde(rename = "vers")]
	pub version: String,
	#[serde(rename = "deps")]
	pub dependencies: Vec<Dependency>,
	#[serde(rename = "cksum")]
	pub checksum_sha256: String,
	pub features: BTreeMap<String, Vec<String>>,
	pub yanked: bool,
	pub links: Option<String>
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
	pub name: String,
	#[serde(rename = "version_req")]
	pub version: String,
	pub features: Vec<String>,
	pub optional: bool,
	pub default_features: bool,
	pub target: Option<String>,
	pub kind: DependencyKind,
	pub registry: Option<String>,
	pub package: Option<String>,
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
	Normal,
	Build,
	Dev,
}

pub fn read_index<R: std::io::Read>(read: R) -> Result<Vec<Entry>, Error> {
	use std::io::BufRead;
	let read = std::io::BufReader::new(read);
	read.lines().map(|line| -> Result<Entry, Error> {
		let line = line.map_err(|e| Error::new(format!("failed to read index: {}", e)))?;
		Entry::from_json(&line)
	}).collect()
}

pub fn write_index<'a, W: std::io::Write>(mut write: W, path: impl AsRef<Path>, entries: impl IntoIterator<Item = &'a Entry>) -> Result<(), Error> {
	let path = path.as_ref();
	for entry in entries.into_iter() {
		let json = serde_json::to_string(entry)
			.map_err(|e| Error::new(format!("failed to serialize index entry {}-{}: {}", entry.name, entry.version, e)))?;
		write.write_all(json.as_bytes())
			.map_err(|e| Error::new(format!("failed to write to {}: {}", path.display(), e)))?;
		write.write_all(b"\n")
			.map_err(|e| Error::new(format!("failed to write to {}: {}", path.display(), e)))?;
	}
	write.flush().map_err(|e| Error::new(format!("failed to write to {}: {}", path.display(), e)))?;
	Ok(())
}

impl Entry {
	pub(crate) fn from_json(data: &str) -> Result<Self, Error> {
		serde_json::from_str(data)
			.map_err(|e| Error::new(format!("failed to parse index entry: {}", e)))
	}

	pub(crate) fn from_manifest(manifest: Manifest, checksum_sha256: String) -> Result<Self, Error> {
		let mut dependencies = Vec::new();
		add_deps(&mut dependencies, manifest.dependencies, DependencyKind::Normal, None)?;
		add_deps(&mut dependencies, manifest.dev_dependencies, DependencyKind::Dev, None)?;
		add_deps(&mut dependencies, manifest.build_dependencies, DependencyKind::Build, None)?;

		for (target, data) in manifest.target {
			add_deps(&mut dependencies, data.dependencies, DependencyKind::Normal, Some(&target))?;
			add_deps(&mut dependencies, data.dev_dependencies, DependencyKind::Dev, Some(&target))?;
			add_deps(&mut dependencies, data.build_dependencies, DependencyKind::Build, Some(&target))?;
		}

		Ok(Self {
			name: manifest.package.name,
			version: manifest.package.version,
			checksum_sha256,
			features: manifest.features,
			yanked: false,
			links: manifest.links,
			dependencies,
		})
	}
}

fn add_deps(out: &mut Vec<Dependency>, deps: BTreeMap<String, ManifestDependency>, kind: DependencyKind, target: Option<&str>) -> Result<(), Error> {
	out.reserve(deps.len());
	for (name, data) in deps {
		out.push(Dependency {
			name,
			version: data.version,
			optional: data.optional,
			features: data.features,
			default_features: data.default_features,
			target: target.map(|x| x.to_string()),
			kind,
			registry: data.registry,
			package: data.package,
		})
	}

	Ok(())
}
