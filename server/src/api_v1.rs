use hyper::{header, Method};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::Registry;
use crate::server::{self, Request, Response, HttpError};

pub async fn handle_request(registry: Arc<RwLock<Registry>>, request: Request, api_path: &str) -> Result<Response, HttpError> {
	if let Some(api_path) = api_path.strip_prefix("crates/") {
		handle_crate_request(registry, request, api_path).await
	} else {
		server::not_found()
	}
}

async fn handle_crate_request(registry: Arc<RwLock<Registry>>, request: Request, api_path: &str) -> Result<Response, HttpError> {
	if api_path == "new" {
		publish_crate(registry, request).await
	} else {
		let (name, rest) = match api_path.split_once('/') {
			Some(x) => x,
			None => return server::not_found(),
		};
		let (version, action) = match rest.split_once('/') {
			Some(x) => x,
			None => return server::not_found(),
		};
		match action {
			"yank" => yank_crate(registry, name, version, request.method()),
			"unyank" => unyank_crate(registry, name, version, request.method()),
			_ => server::not_found()
		}
	}
}

async fn publish_crate(registry: Arc<RwLock<Registry>>, request: Request) -> Result<Response, HttpError> {
	use sha2::Digest;

	if let Some(response) = server::check_supported_method(request.method(), &[Method::PUT]) {
		return response;
	}

	let body = match server::collect_body(request.into_body()).await {
		Ok(x) => x,
		Err(e) => {
			log::error!("Failed to read request body: {}", e);
			return server::internal_server_error("Failed to read response body");
		}
	};

	let (metadata, crate_data) = match parse_crate(&body) {
		Ok(x) => x,
		Err(e) => {
			log::error!("Failed to parse request body: {}", e);
			return error_response(e);
		}
	};

	let crate_sha256 = format!("{:x}", sha2::Sha256::digest(crate_data));
	let index_entry = metadata.into_index_entry(crate_sha256);

	let mut registry = registry.write().unwrap();
	match registry.add_crate_with_metadata(&index_entry, crate_data) {
		Ok(()) => (),
		Err(e) => {
			log::error!("Failed to publish crate {}-{}: {}", index_entry.name, index_entry.version, e);
			return error_response(e);
		},
	}

	log::info!("Published {}-{} with sha256 checksum {}", index_entry.name, index_entry.version, index_entry.checksum_sha256);
	json_response("{\"warnings\":{\"invalid_categories\":[],\"invalid_badges\":[],\"other\":[]}}")
}

#[derive(serde::Deserialize)]
struct NewCrateMeta {
	name: String,

	#[serde(rename = "vers")]
	version: String,

	#[serde(rename = "deps")]
	dependencies: Vec<NewCrateDependency>,

	features: BTreeMap<String, Vec<String>>,

	links: Option<String>

	// Other fields ignored, because not needed for the index.
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NewCrateDependency {
	pub name: String,
	#[serde(rename = "version_req")]
	pub version: String,
	pub features: Vec<String>,
	pub optional: bool,
	pub default_features: bool,
	pub target: Option<String>,
	pub kind: palletizer::index::DependencyKind,
	pub registry: Option<String>,
	pub explicit_name_in_toml: Option<String>,
}

impl NewCrateDependency {
	fn into_index_dependency(self) -> palletizer::index::Dependency {
		// Web API has flipped meaning for the `name` field for renamed dependencies.
		// In the request body, it is the name of the actual package.
		// In the index, it is the name after renaming.
		let package;
		let name;
		if let Some(renamed) = self.explicit_name_in_toml {
			name = renamed;
			package = Some(self.name);
		} else {
			name = self.name;
			package = None;
		};

		palletizer::index::Dependency {
			name,
			version: self.version,
			features: self.features,
			optional: self.optional,
			default_features: self.default_features,
			target: self.target,
			kind: self.kind,
			registry: self.registry,
			package,
		}
	}
}

impl NewCrateMeta {
	fn into_index_entry(self, crate_sha256: String) -> palletizer::index::Entry {
		let dependencies = self.dependencies
			.into_iter()
			.map(NewCrateDependency::into_index_dependency)
			.collect();
		palletizer::index::Entry {
			name: self.name,
			version: self.version,
			dependencies,
			features: self.features,
			checksum_sha256: crate_sha256,
			yanked: false,
			links: self.links,
		}
	}
}

fn parse_crate(data: &[u8]) -> Result<(NewCrateMeta, &[u8]), String> {
	if data.len() < 4 {
		return Err("missing metadata JSON length".into());
	}

	let json_len = usize::from(data[0]) + (usize::from(data[1])<< 8) + (usize::from(data[2]) << 16) + (usize::from(data[3]) << 24);
	let data = &data[4..];

	if data.len() < json_len {
		return Err(format!("expected {} bytes of metadata, got only {} bytes remaining", json_len, data.len()));
	}

	let (json, data) = data.split_at(json_len);

	if data.len() < 4 {
		return Err("missing crate tarball length".into());
	}
	let tarball_len = usize::from(data[0]) + (usize::from(data[1])<< 8) + (usize::from(data[2]) << 16) + (usize::from(data[3]) << 24);
	let tarball = &data[4..];

	if tarball.len() != tarball_len {
		return Err(format!("expected {} exactly bytes of crate tarball, got {} bytes remaining", tarball_len, tarball.len()));
	}

	let meta = serde_json::from_slice(json)
		.map_err(|e| format!("failed to parse crate metadata: {}", e))?;

	Ok((meta, tarball))
}

fn yank_crate(registry: Arc<RwLock<Registry>>, name: &str, version: &str, method: &Method) -> Result<Response, HttpError> {
	if let Some(response) = server::check_supported_method(method, &[Method::DELETE]) {
		return response;
	}

	let mut registry = registry.write().unwrap();
	match registry.yank_crate(name, version) {
		Err(e) => {
			log::info!("Failed to yank {}-{}: {}", name, version, e);
			error_response(e)
		},
		Ok(true) => {
			log::info!("Yanked {}-{}", name, version);
			json_response("{\"ok\":true}")
		},
		Ok(false) => {
			log::info!("Ignored yank request for {}-{} (already yanked)", name, version);
			json_response("{\"ok\":true}")
		},
	}
}

fn unyank_crate(registry: Arc<RwLock<Registry>>, name: &str, version: &str, method: &Method) -> Result<Response, HttpError> {
	if let Some(response) = server::check_supported_method(method, &[Method::PUT]) {
		return response;
	}

	let mut registry = registry.write().unwrap();
	match registry.unyank_crate(name, version) {
		Err(e) => {
			log::info!("Failed to yank {}-{}: {}", name, version, e);
			error_response(e)
		},
		Ok(true) => {
			log::info!("Unyanked {}-{}", name, version);
			json_response("{\"ok\":true}")
		},
		Ok(false) => {
			log::info!("Ignored unyank request for {}-{} (not yanked)", name, version);
			json_response("{\"ok\":true}")
		},
	}
}

fn error_response(message: impl std::fmt::Display) -> Result<Response, HttpError> {
	#[derive(serde::Serialize)]
	struct ErrorResponse {
		errors: Vec<Error>,
	}

	#[derive(serde::Serialize)]
	struct Error {
		detail: String,
	}

	let response = ErrorResponse {
		errors: vec![
			Error { detail: message.to_string() },
		],
	};

	let body = serde_json::to_vec(&response).unwrap();
	json_response(body)
}

fn json_response(json: impl Into<hyper::Body>) -> Result<Response, HttpError> {
	server::response_no_cache()
		.header(header::CONTENT_TYPE, "application/json")
		.body(json.into())
}
