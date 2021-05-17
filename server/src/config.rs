use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
	/// The registry root directory.
	#[serde(default = "default_registry")]
	pub registry: PathBuf,

	#[serde(rename = "listener")]
	pub listeners: Vec<Listener>,
}

fn default_registry() -> PathBuf {
	PathBuf::from(".")
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Listener {
	/// The bind address for this listener.
	pub bind: String,

	/// TLS options.
	#[cfg(feature = "tls")]
	pub tls: Option<Tls>,
}

#[cfg(feature = "tls")]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Tls {
	/// The path to the private key in PEM form.
	pub private_key: PathBuf,

	/// The path to the certificate chain in PEM form.
	///
	/// The chain should start with the leaf certificate,
	/// followed by all parent certificates in order,
	/// up to a certificate signed by a trusted root.
	pub certificate_chain: PathBuf,
}
