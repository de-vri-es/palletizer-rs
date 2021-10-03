use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, Instant};

use openssl::ssl::{Ssl, SslContext};
use tokio::net::TcpStream;
use tokio_openssl::SslStream;

use crate::config;

const RELOAD_AFTER_SUCCESS: Duration = Duration::from_secs(3600 * 24);
const RELOAD_AFTER_ERROR: Duration = Duration::from_secs(60);
const RELOAD_AFTER_ERROR_MAX: Duration = Duration::from_secs(3600);

pub struct TlsAcceptor {
	certificate_chain: PathBuf,
	private_key: PathBuf,
	context: SslContext,
	next_reload: Instant,
	fail_timeout: Duration,
}

impl TlsAcceptor {
	/// Create an acceptor from a configuration.
	pub fn from_config(config: &config::Tls, config_dir: &Path) -> Result<Self, ()> {
		let certificate_chain = config_dir.join(&config.certificate_chain);
		let private_key = config_dir.join(&config.private_key);
		let context = load_tls_files(&certificate_chain, &private_key)?;
		Ok(Self {
			certificate_chain,
			private_key,
			context,
			next_reload: Instant::now() + RELOAD_AFTER_SUCCESS,
			fail_timeout: RELOAD_AFTER_ERROR,
		})
	}

	/// Reload the certificate chain and private key from disk.
	pub fn reload(&mut self) -> Result<(), ()> {
		match load_tls_files(&self.certificate_chain, &self.private_key) {
			Ok(context) => {
				self.context = context;
				self.next_reload = Instant::now() + RELOAD_AFTER_SUCCESS;
				self.fail_timeout = RELOAD_AFTER_ERROR;
				Ok(())
			},
			Err(e) => {
				self.next_reload = Instant::now() + self.fail_timeout;
				self.fail_timeout = (self.fail_timeout * 2).min(RELOAD_AFTER_ERROR_MAX);
				Err(e)
			}
		}
	}

	/// Initialize a TLS session for an accepted connection.
	///
	/// This will automatically reload the TLS key and certificate every 24 hours.
	pub async fn accept(&mut self, connection: TcpStream) -> Result<SslStream<TcpStream>, ()> {
		if Instant::now() >= self.next_reload {
			log::info!("Reloading TLS private key from {}", self.private_key.display());
			log::info!("Reloading TLS certificate from {}", self.certificate_chain.display());
			self.reload().ok();
		}

		let ssl = Ssl::new(&self.context)
			.map_err(|e| log::error!("Failed to initialize TLS session: {}", e))?;
		let mut stream = tokio_openssl::SslStream::new(ssl, connection)
			.map_err(|e| log::error!("Failed to create TLS stream: {}", e))?;
		Pin::new(&mut stream).accept()
			.await
			.map_err(|e| log::error!("TLS handshake failed: {}", e))?;
		Ok(stream)
	}
}

fn load_tls_files(certificate_chain: &Path, private_key: &Path) -> Result<openssl::ssl::SslContext, ()> {
	let mut builder = mozilla_modern_v5()
		.map_err(|e| log::error!("Failed to create OpenSSL context: {}", e))?;
	builder.set_private_key_file(private_key, openssl::ssl::SslFiletype::PEM)
		.map_err(|e| log::error!("Failed to load private key from {}: {}", private_key.display(), e))?;
	builder.set_certificate_chain_file(certificate_chain)
		.map_err(|e| log::error!("Failed to load certificate chain from {}: {}", certificate_chain.display(), e))?;
	Ok(builder.build())
}

fn mozilla_modern_v5() -> Result<openssl::ssl::SslContextBuilder, openssl::error::ErrorStack> {
	use openssl::ssl::{SslMethod, SslOptions};
	let mut context = SslContext::builder(SslMethod::tls_server())?;
	context.set_options(SslOptions::NO_SSL_MASK & !SslOptions::NO_TLSV1_3);
	context.set_ciphersuites(
		"TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256",
	)?;
	Ok(context)
}
