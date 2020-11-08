use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
	Normal,
	Build,
	Dev,
}
