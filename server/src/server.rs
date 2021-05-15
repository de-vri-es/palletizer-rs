use palletizer::Registry;
use std::sync::{Arc, RwLock};
use hyper::{header, StatusCode, Method};
use crate::api_v1;

pub use hyper::http::Error as HttpError;
pub type Request = hyper::Request<hyper::Body>;
pub type Response = hyper::Response<hyper::Body>;

pub async fn handle_request(registry: Arc<RwLock<Registry>>, request: Request) -> Result<Response, HttpError> {
	if let Some(path) = request.uri().path().strip_prefix("/crates/") {
		get_crate(registry, path, request.method())
	} else if let Some(api_path) = request.uri().path().strip_prefix("/api/v1/").map(|x| x.to_owned()) {
		api_v1::handle_request(registry, request, &api_path).await
	} else {
		not_found()
	}
}

fn get_crate(registry: Arc<RwLock<Registry>>, path: &str, method: &Method) -> Result<Response, HttpError> {
	if let Some(response) = check_supported_method(method, &[Method::GET, Method::HEAD]) {
		return response;
	}

	let registry = registry.read().unwrap();
	let crate_path = registry.crate_dir().join(path);
	let data = match std::fs::read(&crate_path) {
		Ok(data) => data,
		Err(e) => {
			return match e.kind() {
				std::io::ErrorKind::NotFound => not_found(),
				std::io::ErrorKind::PermissionDenied => unauthorized(),
				_ => {
					log::error!("Failed to read crate data: {}: {}", crate_path.display(), e);
					internal_server_error("Failed to read crate data")
				}
			};
		},
	};

	let response = hyper::Response::builder()
		.header(header::CACHE_CONTROL, "private") //TODO: Allow for a config option to make this public.
		.header(header::CONTENT_TYPE, "application/gzip");

	if method == Method::GET {
		response.body(data.into())
	} else {
		response.body("".into())
	}
}

pub fn response_no_cache() -> hyper::http::response::Builder {
	hyper::Response::builder()
		.header(header::CACHE_CONTROL, "no-store")
}

pub fn not_found() -> Result<Response, HttpError> {
	response_no_cache()
		.status(StatusCode::NOT_FOUND)
		.body("Not Found".into())
}

pub fn unauthorized() -> Result<Response, HttpError> {
	response_no_cache()
		.status(StatusCode::UNAUTHORIZED)
		.body("Unauthorized".into())
}

pub fn internal_server_error(message: impl std::fmt::Display) -> Result<Response, HttpError> {
	response_no_cache()
		.status(StatusCode::INTERNAL_SERVER_ERROR)
		.body(message.to_string().into())
}

pub fn check_supported_method(actual_method: &Method, allowed_methods: &[Method]) -> Option<Result<Response, HttpError>> {
	if allowed_methods.contains(&actual_method) {
		None
	} else {
		let mut message = String::from("Method not supported. Allowed methods:");
		for (i, method) in allowed_methods.iter().enumerate() {
			if i == 0 {
				message += &format!(" {}", method);
			} else {
				message += &format!(", {}", method);
			}
		}
		Some(
			hyper::Response::builder()
				.status(StatusCode::METHOD_NOT_ALLOWED)
				.body(message.into())
		)
	}
}

pub async fn collect_body(mut body: hyper::Body) -> hyper::Result<Vec<u8>>  {
	use futures::stream::StreamExt;
	let mut data = Vec::new();
	while let Some(chunk) = body.next().await {
		data.extend(chunk?);
	}
	Ok(data)
}
