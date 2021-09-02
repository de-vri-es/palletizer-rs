use palletizer::Registry;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use structopt::StructOpt;
use structopt::clap::AppSettings;

mod api_v1;
mod config;
mod git;
mod logging;
mod server;

#[derive(StructOpt)]
#[structopt(setting = AppSettings::ColoredHelp)]
#[structopt(setting = AppSettings::UnifiedHelpMessage)]
#[structopt(setting = AppSettings::DeriveDisplayOrder)]
struct Options {
	/// Show more messages. Pass twice for even more messages.
	#[structopt(long, short)]
	#[structopt(parse(from_occurrences))]
	verbose: i8,

	/// Show less messages. Pass twice for even less messages.
	#[structopt(long, short)]
	#[structopt(parse(from_occurrences))]
	quiet: i8,

	/// The configuration file to use.
	config: PathBuf,
}

impl Options {
	fn load_config(&self) -> Result<config::Config, ()> {
		let data = std::fs::read(&self.config)
			.map_err(|e| log::error!("Failed to read {}: {}", self.config.display(), e))?;
		toml::from_slice(&data)
			.map_err(|e| log::error!("Failed to parse {}: {}", self.config.display(), e))
	}
}

fn main() {
	let options = Options::from_args();
	logging::init(module_path!(), &[], options.verbose - options.quiet);
	if let Err(()) = do_main(options) {
		std::process::exit(1);
	}
}

fn do_main(options: Options) -> Result<(), ()> {
	let config_dir = options.config.parent()
		.ok_or_else(|| log::error!("Failed to determine parent directory of config file"))?;
	let config = options.load_config()?;
	let registry = Registry::open(config_dir.join(&config.registry))
		.map_err(|e| log::error!("{}", e))?;
	let index_repo_path = registry.index_dir();
	let registry = Arc::new(RwLock::new(registry));

	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.map_err(|e| log::error!("Failed to initialize I/O runtime: {}", e))?;

	runtime.block_on(async move {
		let mut futures = Vec::new();
		for listener in config.listeners {
			futures.push(run_server(registry.clone(), index_repo_path.clone(), config_dir.to_path_buf(), listener));
		}
		futures::future::try_join_all(futures).await?;
		Ok(())
	})
}

async fn run_server(registry: Arc<RwLock<Registry>>, index_repo_path: PathBuf, config_dir: PathBuf, config: config::Listener) -> Result<(), ()> {
	let listener = tokio::net::TcpListener::bind(&config.bind)
		.await
		.map_err(|e| log::error!("Failed to listen on {}: {}", &config.bind, e))?;
	log::info!("Server listening on {}", config.bind);

	#[cfg(feature = "tls")]
	let tls_context = match config.tls.as_ref() {
		None => None,
		Some(tls) => {
			let mut context = mozilla_modern_v5()
				.map_err(|e| log::error!("Failed to create OpenSSL context: {}", e))?;
			context.set_certificate_chain_file(config_dir.join(&tls.certificate_chain))
				.map_err(|e| log::error!("Failed to load certificate chain: {}", e))?;
			context.set_private_key_file(config_dir.join(&tls.private_key), openssl::ssl::SslFiletype::PEM)
				.map_err(|e| log::error!("Failed to load private key: {}", e))?;
			Some(context.build())
		}
	};

	loop {
		let (connection, address) = listener.accept()
			.await
			.map_err(|e| log::error!("Failed to accept connection on {}: {}", &config.bind, e))?;
		log::debug!("Accepted connection from {}", address);

		#[cfg(feature = "tls")]
		if let Some(tls_context) = &tls_context {
			let session = openssl::ssl::Ssl::new(tls_context)
				.map_err(|e| log::error!("Failed to initialize TLS session: {}", e))?;
			let mut connection = tokio_openssl::SslStream::new(session, connection)
				.map_err(|e| log::error!("Failed to initialize TLS stream: {}", e))?;
			std::pin::Pin::new(&mut connection)
				.accept()
				.await
				.map_err(|e| log::error!("Failed to complete TLS handshake: {}", e))?;
			tokio::spawn(serve_connection(connection, address, registry.clone(), index_repo_path.clone()));
			continue;
		}

		tokio::spawn(serve_connection(connection, address, registry.clone(), index_repo_path.clone()));
	}
}

async fn serve_connection<S>(connection: S, address: std::net::SocketAddr, registry: Arc<RwLock<Registry>>, index_repo_path: PathBuf)
where
	S: tokio::io::AsyncRead + tokio::io::AsyncWrite + std::marker::Unpin + 'static,
{
	let result = hyper::server::conn::Http::new()
		.serve_connection(connection, hyper::service::service_fn(move |request| {
			server::handle_request(registry.clone(), index_repo_path.clone(), request)
		}))
		.await;
	if let Err(e) = result {
		let message = e.to_string();
		// EEEW! But hyper forces us to do this :(
		if !message.starts_with("error shutting down connection:") {
			log::error!("Error in connection with {}: {}", address, message);
		}
	}
}

#[cfg(feature = "tls")]
pub fn mozilla_modern_v5() -> Result<openssl::ssl::SslContextBuilder, openssl::error::ErrorStack> {
	use openssl::ssl::{SslContext, SslMethod, SslOptions};
	let mut context = SslContext::builder(SslMethod::tls_server())?;
	context.set_options(SslOptions::NO_SSL_MASK & !SslOptions::NO_TLSV1_3);
	context.set_ciphersuites(
		"TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256",
	)?;
	Ok(context)
}
