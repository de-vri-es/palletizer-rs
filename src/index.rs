use std::collections::BTreeMap;
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
	#[serde(rename = "req")]
	pub requirement: String,
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

impl Entry {
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
			links: manifest.links.clone(),
			dependencies,
		})
	}
}

fn add_deps(out: &mut Vec<Dependency>, deps: BTreeMap<String, ManifestDependency>, kind: DependencyKind, target: Option<&str>) -> Result<(), Error> {
	out.reserve(deps.len());
	for (name, data) in deps {
		out.push(Dependency {
			name,
			requirement: data.version,
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
