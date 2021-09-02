use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The registry configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
	/// The download URL for crates.
	///
	/// See also https://doc.rust-lang.org/cargo/reference/registries.html#index-format
	pub download_url: String,

	/// The API URL for cargo.
	///
	/// See also https://doc.rust-lang.org/cargo/reference/registries.html#index-format
	pub api_url: String,

	/// The path to the index repository.
	///
	/// Relative paths are resolved relative to directory that contains the config file.
	pub index_dir: PathBuf,

	/// The path where crates are stored.
	///
	/// Relative paths are resolved relative to directory that contains the config file.
	pub crate_dir: PathBuf,

	/// Allowed external registries for crates in this registry.
	///
	/// Packages with dependencies from other registries will be refused.
	///
	/// Each entry should be the full URL of the index repository of an external registry.
	#[serde(default = "Default::default", skip_serializing_if = "Vec::is_empty")]
	pub allowed_registries: Vec<String>,
}

impl Config {
	pub fn example() -> Self {
		Self {
			download_url: "https://example.com/crates/{crate}/{crate}-{version}.crate".into(),
			api_url: "https://example.com".into(),
			index_dir: "index".into(),
			crate_dir: "crates".into(),
			allowed_registries: vec![
				"https://github.com/rust-lang/crates.io-index".into(),
			],
		}
	}
}


impl Config {
	/// Encode the configuration as JSON for Cargo.
	pub fn cargo_json(&self) -> String {
		#[derive(Serialize)]
		struct CargoConfig<'a> {
			dl: &'a str,
			api: &'a str,
		}

		let cargo_config = CargoConfig {
			dl: &self.download_url,
			api: &self.api_url,
		};

		// Unwrap should be fine: contents is always JSON encodable.
		let mut json = serde_json::to_string_pretty(&cargo_config).unwrap();
		json.push('\n');
		json
	}
}
