use palletizer::Registry;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

mod api_v1;
mod config;
mod git;
mod logging;
mod server;

#[cfg(feature = "tls")]
mod tls;

#[derive(clap::Parser)]
struct Options {
	/// Show more messages. Pass twice for even more messages.
	#[clap(long, short)]
	#[clap(action = clap::ArgAction::Count)]
	verbose: u8,

	/// Show less messages. Pass twice for even less messages.
	#[clap(long, short)]
	#[clap(action = clap::ArgAction::Count)]
	quiet: u8,

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
	if let Err(()) = do_main(clap::Parser::parse()) {
		std::process::exit(1);
	}
}

fn do_main(options: Options) -> Result<(), ()> {
	logging::init(module_path!(), &[], i16::from(options.verbose) - i16::from(options.quiet));

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
	let mut tls_acceptor = match config.tls.as_ref() {
		None => None,
		Some(tls) => Some(tls::TlsAcceptor::from_config(tls, &config_dir)?),
	};

	loop {
		let (connection, address) = listener.accept()
			.await
			.map_err(|e| log::error!("Failed to accept connection on {}: {}", &config.bind, e))?;
		log::debug!("Accepted connection from {}", address);

		#[cfg(feature = "tls")]
		if let Some(tls_acceptor) = &mut tls_acceptor {
			let connection = tls_acceptor.accept(connection).await?;
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
	let service = hyper::service::service_fn(move |request| {
		server::handle_request(registry.clone(), index_repo_path.clone(), request)
	});
	let connection = hyper_util::rt::TokioIo::new(connection);
	let result = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
		.http1()
		.keep_alive(true)
		.serve_connection(connection, service)
		.await;
	if let Err(e) = result {
		log::error!("Error in connection with {address}: {e}");
	}
}
