use futures::{Stream, TryStreamExt as _};
use http_body_util::BodyExt as _;
use hyper::body::{Bytes, Incoming};
use palletizer::Registry;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use std::task::ready;
use hyper::{header, StatusCode, Method};
use crate::{api_v1, git};

pub use hyper::http::Error as HttpError;
pub type Request = hyper::Request<Incoming>;
pub type Response = hyper::Response<Body>;

pub async fn handle_request(registry: Arc<RwLock<Registry>>, index_repo_path: PathBuf, request: Request) -> Result<Response, HttpError> {
	log::debug!("Got {} request for {}", request.method(), request.uri());
	let path = request.uri().path().replace("//", "/");
	if let Some(path) = path.strip_prefix("/crates/") {
		get_crate(registry, path, request.method())
	} else if let Some(api_path) = path.strip_prefix("/api/v1/") {
		api_v1::handle_request(registry, request, api_path).await
	} else if let Some(path) = path.strip_prefix("/index.git/") {
		git::handle_request(&index_repo_path, request, path).await
	} else if let Some(path) = path.strip_prefix("/index/") {
		git::handle_request(&index_repo_path, request, path).await
	} else {
		not_found()
	}
}

fn get_crate(registry: Arc<RwLock<Registry>>, path: &str, method: &Method) -> Result<Response, HttpError> {
	if let Some(response) = check_supported_method(method, &[Method::GET, Method::HEAD]) {
		log::warn!("Unsupported request method for crate download: {}", method);
		return response;
	}

	let registry = registry.read().unwrap();
	let crate_path = registry.crate_dir().join(path);
	let data = match std::fs::read(&crate_path) {
		Ok(data) => data,
		Err(e) => {
			return match e.kind() {
				std::io::ErrorKind::NotFound => {
					log::warn!("Received request for unknown crate: {}", path);
					not_found()
				},
				std::io::ErrorKind::PermissionDenied => {
					log::error!("Failed to read crate data: {}: {}", crate_path.display(), e);
					unauthorized()
				},
				_ => {
					log::error!("Failed to read crate data: {}: {}", crate_path.display(), e);
					internal_server_error("Failed to read crate data")
				},
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
	if allowed_methods.contains(actual_method) {
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

pub async fn collect_body(mut body: Incoming) -> hyper::Result<Vec<u8>>  {
	let mut data = Vec::new();
	while let Some(frame) = body.frame().await {
		if let Ok(chunk) = frame?.into_data() {
			data.extend_from_slice(&chunk);
		}
	}
	Ok(data)
}

pub enum Body {
	Bytes(http_body_util::Full<Bytes>),
	Stream(StreamBody<Bytes, String>),
}

impl From<&'static str> for Body {
	fn from(value: &'static str) -> Self {
		Self::Bytes(Bytes::from(value).into())
	}
}

impl From<String> for Body {
	fn from(value: String) -> Self {
		Self::Bytes(Bytes::from(value).into())
	}
}

impl From<&'static [u8]> for Body {
	fn from(value: &'static [u8]) -> Self {
		Self::Bytes(Bytes::from(value).into())
	}
}

impl From<Vec<u8>> for Body {
	fn from(value: Vec<u8>) -> Self {
		Self::Bytes(Bytes::from(value).into())
	}
}

impl hyper::body::Body for Body {
	type Data = Bytes;
	type Error = String;

	fn poll_frame(
		self: std::pin::Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
		match self.get_mut() {
			Self::Bytes(x) => match ready!(hyper::body::Body::poll_frame(Pin::new(x), cx)) {
				None => std::task::Poll::Ready(None),
				Some(Ok(x)) => std::task::Poll::Ready(Some(Ok(x))),
			}
			Self::Stream(x) => hyper::body::Body::poll_frame(Pin::new(x), cx),
		}
	}
}

pub struct StreamBody<Buf, Error> {
	frames: Pin<Box<dyn Stream<Item = Result<hyper::body::Frame<Buf>, Error>> + Send>>,
}

impl<Buf, Error> StreamBody<Buf, Error> {
	pub fn new<Frames>(frames: Frames) -> Self
	where
		Frames: Stream<Item = Result<hyper::body::Frame<Buf>, Error>> + Send + 'static,
	{
		Self {
			frames: Box::pin(frames)
		}
	}
}

fn frame_stream<R: tokio::io::AsyncRead + Send + 'static>(stream: R) -> impl Stream<Item = Result<hyper::body::Frame<Bytes>, std::io::Error>> {
		pub struct ReadDataFrame<R> {
			stream: R,
		}
		impl<R: tokio::io::AsyncRead> Stream for ReadDataFrame<R> {
			type Item = Result<hyper::body::Frame<Bytes>, std::io::Error>;

			fn poll_next(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
				let mut buffer = vec![0; 512];
				let read = {
					let mut buffer = tokio::io::ReadBuf::new(&mut buffer);
					let stream = unsafe { self.map_unchecked_mut(|x| &mut x.stream) };
					match ready!(tokio::io::AsyncRead::poll_read(stream, cx, &mut buffer)) {
						Err(e) => return std::task::Poll::Ready(Some(Err(e))),
						Ok(()) => buffer.filled().len(),
					}
				};
				if read == 0 {
					std::task::Poll::Ready(None)
				} else {
					buffer.truncate(read);
					let buffer = Bytes::from(buffer);
					std::task::Poll::Ready(Some(Ok(hyper::body::Frame::data(buffer))))
				}
			}
		}

		ReadDataFrame { stream }
}

impl<R: tokio::io::AsyncRead + Send + 'static> From<R> for StreamBody<Bytes, std::io::Error> {
	fn from(stream: R) -> Self {
		Self::new(frame_stream(stream))
	}
}

impl<R: tokio::io::AsyncRead + Send + 'static> From<R> for StreamBody<Bytes, String> {
	fn from(stream: R) -> Self {
		let stream = frame_stream(stream)
			.map_err(|e| format!("failed to read from stream: {e}"));
		Self::new(stream)
	}
}

impl<Buf: hyper::body::Buf, Error> hyper::body::Body for StreamBody<Buf, Error> {
	type Data = Buf;
	type Error = Error;

	fn poll_frame(
		self: Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
		Stream::poll_next(self.get_mut().frames.as_mut(), cx)
	}
}
