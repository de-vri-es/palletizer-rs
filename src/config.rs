use serde::{Deserialize, Serialize};

/// The registry configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
	/// The download URL for crates.
	///
	/// See also https://doc.rust-lang.org/cargo/reference/registries.html#index-format
	pub download_url: Option<String>,

	/// The API URL for cargo.
	///
	/// See also https://doc.rust-lang.org/cargo/reference/registries.html#index-format
	pub api_url: Option<String>,
}

impl Config {
	pub fn example() -> Self {
		Self {
			download_url: Some("https://example.com/api/v1/crates".into()),
			api_url: Some("https://example/com/".into()),
		}
	}
}
