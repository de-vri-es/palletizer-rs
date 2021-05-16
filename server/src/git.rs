use hyper::{header, Method, StatusCode};
use std::path::Path;
use tokio::process::Command;

use crate::server::{self, Request, Response, HttpError};

/// Handle requests for the git smart HTTP transport.
pub async fn handle_request(repo_path: &Path, request: Request, rel_path: &str) -> Result<Response, HttpError> {
	if rel_path == "info/refs" {
		handle_info(repo_path, request).await
	} else if rel_path == "git-upload-pack" {
		handle_upload_pack(repo_path, request).await
	} else if rel_path == "git-receive-pack" {
		simple_response(StatusCode::FORBIDDEN, "This repository is read-only")
	} else {
		server::not_found()
	}
}

/// Handle requests for 'info/refs'.
///
/// This allows client to discover refs on the server,
/// and to probe for protocol support.
///
/// We refuse everything but requests for the git-upload-pack service.
async fn handle_info(repo_path: &Path, request: Request) -> Result<Response, HttpError> {
	if let Some(response) = server::check_supported_method(request.method(), &[Method::GET]) {
		return response;
	}

	let query = request.uri().query().unwrap_or("");
	if query.is_empty() {
		simple_response(StatusCode::BAD_REQUEST, "Dumb HTTP protocol not supported")
	} else if query == "service=git-receive-pack" {
		simple_response(StatusCode::FORBIDDEN, "This repository is read-only")
	} else if query == "service=git-upload-pack" {
		handle_upload_pack_info(repo_path).await
	} else {
		simple_response(StatusCode::BAD_REQUEST, "Unrecognized query parameters")
	}
}

/// Handle the request for 'info/refs?service=git-upload-pack'.
///
/// This delegates to the system `git` command for the actual work.
async fn handle_upload_pack_info(repo_path: &Path) -> Result<Response, HttpError> {
	// Spawn a child process for the actual work.
	let child = Command::new("git-upload-pack")
		.arg("--advertise-refs")
		.arg(repo_path)
		.stdin(std::process::Stdio::null())
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.spawn();

	let child = match child {
		Ok(x) => x,
		Err(e) => {
			log::error!("failed to run git-upload-pack: {}", e);
			return internal_server_error("internal server error");
		},
	};

	let output = match child.wait_with_output().await {
		Ok(x) => x,
		Err(e) => {
			log::error!("failed to wait for git-upload-pack: {}", e);
			return internal_server_error("internal server error");
		}
	};

	if !output.status.success() {
		for line in output.stderr.split(|&c| c == b'\n') {
			if let Err(line) = std::str::from_utf8(line) {
				log::debug!("git-upload-pack: {}", line);
			}
		}
		log::error!("git-upload-pack --advertise-refs exitted with {:?}", output.status);
		return internal_server_error("internal server error");
	}

	// Prepend the proper prefix for the HTTP protocol.
	let response_prefix = b"001e# service=git-upload-pack\n0000";
	let mut body = Vec::with_capacity(response_prefix.len() + output.stdout.len());
	body.extend_from_slice(response_prefix);
	body.extend_from_slice(&output.stdout);

	// Send the response.
	hyper::Response::builder()
		.header(header::CONTENT_TYPE, "application/x-git-upload-pack-advertisement")
		.header(header::CACHE_CONTROL, "no-store")
		.body(body.into())
}

/// Handle the request for 'git-upload-pack' service.
///
/// This delegates to the system `git` command for the actual work.
async fn handle_upload_pack(repo_path: &Path, mut request: Request) -> Result<Response, HttpError> {
	use futures::StreamExt;
	use tokio::io::AsyncWriteExt;
	use tokio::io::AsyncBufReadExt;

	if let Some(response) = server::check_supported_method(request.method(), &[Method::POST]) {
		return response;
	}

	// Verify the Content-Type of the request.
	if request.headers().get(header::CONTENT_TYPE).map(|x| x != "application/x-git-upload-pack-request").unwrap_or(false) {
		return simple_response(StatusCode::UNSUPPORTED_MEDIA_TYPE, "invalid Content-Type");
	}

	// Spawn a child process for the heavy lifting.
	let child = Command::new("git-upload-pack")
		.arg("--stateless-rpc")
		.arg(repo_path)
		.stdin(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.spawn();

	let mut child = match child {
		Ok(x) => x,
		Err(e) => {
			log::error!("failed to run git-upload-pack: {}", e);
			return internal_server_error("internal server error");
		},
	};

	let mut stdin = child.stdin.take().unwrap();
	let stdout = child.stdout.take().unwrap();
	let stderr = child.stderr.take().unwrap();

	// Forward the request body to the stdin of the child.
	while let Some(chunk) = request.body_mut().next().await {
		let chunk = match chunk {
			Ok(x) => x,
			Err(e) => {
				log::error!("Failed to read body chunk: {}", e);
				return internal_server_error("internal server error");
			}
		};
		if let Err(e) = stdin.write_all(&chunk).await {
			log::error!("Failed to write body chunk to git-upload-pack --stateless-rpc: {}", e);
			return internal_server_error("internal server error");
		}
	}

	// Close the child stdin to ensure it is not waiting for more data.
	drop(stdin);

	// Monitor output and exit status in a background task.
	tokio::spawn(async move {
		let mut stderr = tokio::io::BufReader::new(stderr).lines();
		loop {
			match stderr.next_line().await {
				Ok(None) => break,
				Ok(Some(line)) => log::debug!("git-upload-pack --stateless-rpc: {}", line),
				Err(e) => {
					log::warn!("Failed to read stderr of git-upload-pack --stateless-rpc: {}", e);
					break;
				},
			}
		}
		match child.wait().await {
			Ok(x) => {
				if !x.success() {
					log::error!("Command git-upload-pack --stateless-rpc exitted with {}", x);
				}
			},
			Err(e) => log::error!("Failed to wait for git-upload-pack --stateless-rpc: {}", e),
		}
	});

	// Forward the stdout to the response body.
	hyper::Response::builder()
		.header(header::CONTENT_TYPE, "application/x-git-upload-pack-result")
		.header(header::CACHE_CONTROL, "no-store")
		.body(hyper::Body::wrap_stream(ReadChunks::new(stdout, 512)))
}

/// Create a plain text HTTP response without any specific caching instructions.
fn simple_response(status: StatusCode, message: impl Into<hyper::Body>) -> Result<Response, HttpError> {
	hyper::Response::builder()
		.status(status)
		.header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
		.body(message.into())
}

/// Create a plain text internal server error response that forbids caching.
fn internal_server_error(message: impl Into<hyper::Body>) -> Result<Response, HttpError> {
	hyper::Response::builder()
		.status(StatusCode::INTERNAL_SERVER_ERROR)
		.header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
		.header(header::CACHE_CONTROL, "no-store")
		.body(message.into())
}


/// Stream of chunks read from an [`tokio::io::AsyncRead`].
struct ReadChunks<R> {
	/// The stream being read from.
	read_stream: R,

	/// The maximum chunk size.
	max_chunk_size: usize,

	/// The temporary buffer for reading chunks.
	buffer: Vec<u8>,
}

impl<R> ReadChunks<R> {
	/// Wrap a [`tokio::io::AsyncRead`] in a [`ReadChunks`].
	pub fn new(read_stream: R, max_chunk_size: usize) -> Self {
		Self {
			read_stream,
			max_chunk_size,
			buffer: vec![0; max_chunk_size],
		}
	}

	/// Take the current buffer and replace it with a new one.
	///
	/// This resizes the temporary buffer to `valid` bytes and returns it.
	/// A new buffer is created for the next read.
	fn take_buffer(&mut self, valid: usize) -> Vec<u8> {
		self.buffer.resize(valid, 0);
		std::mem::replace(&mut self.buffer, vec![0; self.max_chunk_size])
	}
}

impl<R: tokio::io::AsyncRead + std::marker::Unpin> futures::stream::Stream for ReadChunks<R> {
	type Item = std::io::Result<hyper::body::Bytes>;

	fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context) -> std::task::Poll<Option<Self::Item>> {
		let me = self.get_mut();
		let mut buffer = tokio::io::ReadBuf::new(&mut me.buffer);
		match std::pin::Pin::new(&mut me.read_stream).poll_read(cx, &mut buffer)? {
			std::task::Poll::Ready(()) => (),
			std::task::Poll::Pending => return std::task::Poll::Pending,
		};

		let read = buffer.filled().len();
		if read == 0 {
			std::task::Poll::Ready(None)
		} else {
			std::task::Poll::Ready(Some(Ok(me.take_buffer(read).into())))
		}
	}
}
