use hyper::{header, Method};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::Registry;
use crate::server::{self, Request, Response, HttpError};

pub async fn handle_request(registry: Arc<RwLock<Registry>>, request: Request, api_path: &str) -> Result<Response, HttpError> {
	if api_path == "crates" {
		search(registry, request.uri().query())
	} else if let Some(api_path) = api_path.strip_prefix("crates/") {
		handle_crate_request(registry, request, api_path).await
	} else {
		log::warn!("Got request for unknown or unimplemented API V1 endpoint: {}", api_path);
		server::not_found()
	}
}

async fn handle_crate_request(registry: Arc<RwLock<Registry>>, request: Request, api_path: &str) -> Result<Response, HttpError> {
	if api_path == "new" {
		publish_crate(registry, request).await
	} else {
		let (name, rest) = match api_path.split_once('/') {
			Some(x) => x,
			None => {
				log::warn!("Failed to determine crate name from API url: {}", api_path);
				return server::not_found();
			},
		};
		let (version, action) = match rest.split_once('/') {
			Some(x) => x,
			None => {
				log::warn!("Failed to determine crate action from API url: {}", api_path);
				return server::not_found();
			},
		};
		match action {
			"yank" => yank_crate(registry, name, version, request.method()),
			"unyank" => unyank_crate(registry, name, version, request.method()),
			_ => {
				log::warn!("Got request for unknown or unimplemented crate action: {}", action);
				server::not_found()
			},
		}
	}
}

async fn publish_crate(registry: Arc<RwLock<Registry>>, request: Request) -> Result<Response, HttpError> {
	use sha2::Digest;

	if let Some(response) = server::check_supported_method(request.method(), &[Method::PUT]) {
		log::warn!("Unsupported request method for v1/crates/new: {}", request.method());
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

fn search(registry: Arc<RwLock<Registry>>, url_query: Option<&str>) -> Result<Response, HttpError> {
	#[derive(serde::Deserialize)]
	struct Params<'a> {
		q: Option<Cow<'a, str>>,
		per_page: Option<usize>,
	}

	#[derive(serde::Serialize)]
	struct FoundCrate {
		name: String,
		max_version: String,
		description: String,
	}

	#[derive(serde::Serialize)]
	struct SearchResultsMeta {
		total: usize,
	}

	#[derive(serde::Serialize)]
	struct SearchResults {
		crates: Vec<FoundCrate>,
		meta: SearchResultsMeta,
	}

	let params: Params = match serde_urlencoded::from_str(url_query.unwrap_or("")) {
		Err(e) => return error_response(e),
		Ok(params) => params,
	};

	let query = params.q.unwrap_or_else(|| "".into());
	let query = query.as_ref();
	let max_results = params.per_page.unwrap_or(10);

	let registry = registry.read().unwrap();

	let mut crates: Vec<_> = registry.iter_crate_names()
		.filter_map(|name| {
			let name = match name {
				Ok(x) => x,
				Err(e) => {
					log::warn!("{}", e);
					return None;
				},
			};
			if !name.contains(&query) {
				return None;
			}
			let entries = match registry.read_index(&name) {
				Ok(x) => x,
				Err(e) => {
					log::warn!("{}", e);
					return None;
				}
			};

			entries
				.iter()
				.filter_map(|entry| semver::Version::parse(&entry.version).ok())
				.max_by_key(|version| version.clone())
				.map(|version| {
					FoundCrate {
						name,
						max_version: version.to_string(),
						description: "".into(), // TODO: omfg, got to read the compressed crate file to extract the manifest
					}
				})
		})
		.collect();

	let total = crates.len();
	crates.truncate(max_results);

	let json = serde_json::to_string(&SearchResults {
		crates,
		meta: SearchResultsMeta {
			total,
		}
	}).unwrap();

	json_response(json)
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
