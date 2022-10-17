use palletizer::Registry;
use std::path::PathBuf;

#[derive(clap::Parser)]
#[clap(setting = clap::AppSettings::DeriveDisplayOrder)]
#[clap(version)]
struct Options {
	/// The command to run.
	#[clap(subcommand)]
	command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
	Init(Init),
	Add(AddCrate),
	Delete(DeleteCrate),
	Yank(YankCrate),
	Unyank(UnyankCrate),
}

/// Initialize a new registry.
#[derive(clap::Parser)]
#[clap(setting = clap::AppSettings::DeriveDisplayOrder)]
#[clap(version)]
struct Init {
	/// The path of the registry to initialize.
	#[clap(long, short)]
	#[clap(default_value = ".")]
	registry: PathBuf,

	/// The URL of the server.
	#[clap(long, short)]
	url: String,

	/// Directory to store the index repository.
	#[clap(long)]
	#[clap(default_value = "index")]
	index_dir: PathBuf,

	/// Directory to store added crates.
	#[clap(long)]
	#[clap(default_value = "crates")]
	crate_dir: PathBuf,

	/// Custom allowed registries for dependencies.
	#[clap(long = "allowed-registry")]
	allowed_registries: Vec<String>,

	/// Do not automatically allow dependencies from crates.io.
	#[clap(long)]
	no_crates_io: bool,
}

/// Add a crate to the registry.
#[derive(clap::Parser)]
#[clap(setting = clap::AppSettings::DeriveDisplayOrder)]
#[clap(version)]
struct AddCrate {
	/// The root of of registry to work on.
	#[clap(long, short)]
	#[clap(default_value = ".")]
	registry: PathBuf,

	/// The packaged crate file to add.
	crate_file: PathBuf,
}

/// Completely delete a crate from the registry.
#[derive(clap::Parser)]
#[clap(setting = clap::AppSettings::DeriveDisplayOrder)]
#[clap(version)]
struct DeleteCrate {
	/// The root of of registry to work on.
	#[clap(long, short)]
	#[clap(default_value = ".")]
	registry: PathBuf,

	/// The name of the crate to delete.
	name: String,
}

/// Yank a crate version from the registry.
#[derive(clap::Parser)]
#[clap(setting = clap::AppSettings::DeriveDisplayOrder)]
#[clap(version)]
struct YankCrate {
	/// The root of of registry to work on.
	#[clap(long, short)]
	#[clap(default_value = ".")]
	registry: PathBuf,

	/// The name of the crate to yank.
	name: String,

	/// The version to yank.
	version: String,
}

/// Unyank a crate version from the registry.
#[derive(clap::Parser)]
#[clap(setting = clap::AppSettings::DeriveDisplayOrder)]
#[clap(version)]
struct UnyankCrate {
	/// The root of of registry to work on.
	#[clap(long, short)]
	#[clap(default_value = ".")]
	registry: PathBuf,

	/// The name of the crate to yank.
	name: String,

	/// The version to yank.
	version: String,
}

fn main() {
	if do_main(clap::Parser::parse()).is_err() {
		std::process::exit(1);
	}
}

fn do_main(options: Options) -> Result<(), ()> {
	match &options.command {
		Command::Init(command) => init(command),
		Command::Add(command) => add_crate(command),
		Command::Delete(command) => delete_crate(command),
		Command::Yank(command) => yank_crate(command),
		Command::Unyank(command) => unyank_crate(command),
	}
}

fn init(command: &Init) -> Result<(), ()> {
	let download_url = format!("{}/crates/{{crate}}/{{crate}}-{{version}}.crate", command.url);
	let api_url = command.url.clone();

	let mut allowed_registries = Vec::with_capacity(command.allowed_registries.len() + 1);
	if !command.no_crates_io {
		allowed_registries.push(String::from("https://github.com/rust-lang/crates.io-index"));
	}
	allowed_registries.extend_from_slice(&command.allowed_registries);

	let config = palletizer::Config {
		download_url,
		api_url,
		index_dir: command.index_dir.clone(),
		crate_dir: command.crate_dir.clone(),
		allowed_registries: command.allowed_registries.clone(),
	};

	let registry = Registry::init(&command.registry, config)
		.map_err(|e| eprintln!("{}", e))?;

	println!("Sucessfully initialized registry.");
	println!();
	println!("To use the registry, add this to your Cargo configuration (for example `$HOME/.cargo/config`):");
	println!();
	println!("[registries]");
	println!("my-registry = {{ index = \"{url}/index\" }}", url = registry.api_url());

	Ok(())
}

fn add_crate(command: &AddCrate) -> Result<(), ()> {
	let mut registry = Registry::open(&command.registry)
		.map_err(|e| eprintln!("{}", e))?;
	registry.add_crate_from_file(&command.crate_file)
		.map_err(|e| eprintln!("{}", e))?;
	Ok(())
}

fn delete_crate(command: &DeleteCrate) -> Result<(), ()> {
	let mut registry = Registry::open(&command.registry)
		.map_err(|e| eprintln!("{}", e))?;
	registry.delete_crate(&command.name)
		.map_err(|e| eprintln!("{}", e))?;
	Ok(())
}

fn yank_crate(command: &YankCrate) -> Result<(), ()> {
	let mut registry = Registry::open(&command.registry)
		.map_err(|e| eprintln!("{}", e))?;
	registry.yank_crate(&command.name, &command.version)
		.map_err(|e| eprintln!("{}", e))?;
	Ok(())
}

fn unyank_crate(command: &UnyankCrate) -> Result<(), ()> {
	let mut registry = Registry::open(&command.registry)
		.map_err(|e| eprintln!("{}", e))?;
	registry.unyank_crate(&command.name, &command.version)
		.map_err(|e| eprintln!("{}", e))?;
	Ok(())
}
