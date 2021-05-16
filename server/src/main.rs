use palletizer::Registry;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use structopt::StructOpt;
use structopt::clap::AppSettings;

mod api_v1;
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

	/// The root of of registry.
	#[structopt(long, short)]
	#[structopt(default_value = ".")]
	registry: PathBuf,

	/// The address to bind to.
	#[structopt(long, short)]
	#[structopt(default_value = "[::]:8080")]
	bind: String,
}

fn main() {
	let options = Options::from_args();
	logging::init(env!("CARGO_CRATE_NAME"), options.verbose);
	if let Err(()) = do_main(options) {
		std::process::exit(1);
	}
}

fn do_main(options: Options) -> Result<(), ()> {
	let registry = Registry::open(&options.registry)
		.map_err(|e| log::error!("{}", e))?;
	let index_repo_path = registry.index_dir();
	let registry = Arc::new(RwLock::new(registry));

	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.map_err(|e| log::error!("Failed to initialize I/O runtime: {}", e))?;

	runtime.block_on(run_server(registry, index_repo_path, options))
}

async fn run_server(registry: Arc<RwLock<Registry>>, index_repo_path: PathBuf, options: Options) -> Result<(), ()> {
	let listener = tokio::net::TcpListener::bind(&options.bind)
		.await
		.map_err(|e| log::error!("Failed to listen on {}: {}", &options.bind, e))?;
	log::info!("Server listening on {}", options.bind);

	loop {
		let (connection, addr) = listener.accept()
			.await
			.map_err(|e| log::error!("Failed to accept connection on {}: {}", &options.bind, e))?;
		log::debug!("Accepted connection from {}", addr);

		let registry = registry.clone();
		let index_repo_path = index_repo_path.clone();
		tokio::spawn(async move {
			let result = hyper::server::conn::Http::new()
				.serve_connection(connection, hyper::service::service_fn(move |request| {
					server::handle_request(registry.clone(), index_repo_path.clone(), request)
				}))
				.await;
			if let Err(e) = result {
				let message = e.to_string();
				// EEEW! But hyper forces us to do this :(
				if !message.starts_with("error shutting down connection:") {
					log::error!("Error in connection with {}: {}", addr, message);
				}
			}
		});
	}
}
