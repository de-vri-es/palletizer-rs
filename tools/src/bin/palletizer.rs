use palletizer::Registry;
use std::path::PathBuf;
use structopt::StructOpt;
use structopt::clap::AppSettings;

#[derive(StructOpt)]
#[structopt(setting = AppSettings::ColoredHelp)]
#[structopt(setting = AppSettings::UnifiedHelpMessage)]
#[structopt(setting = AppSettings::DeriveDisplayOrder)]
struct Options {
	/// The command to run.
	#[structopt(subcommand)]
	command: Command,
}

#[derive(StructOpt)]
enum Command {
	Init(Init),
	Add(AddCrate),
	Delete(DeleteCrate),
	Yank(YankCrate),
	Unyank(UnyankCrate),
}

/// Initialize a new registry.
#[derive(StructOpt)]
#[structopt(setting = AppSettings::ColoredHelp)]
#[structopt(setting = AppSettings::UnifiedHelpMessage)]
#[structopt(setting = AppSettings::DeriveDisplayOrder)]
struct Init {
	/// The path of the registry to initialize.
	#[structopt(long, short)]
	#[structopt(default_value = ".")]
	registry: PathBuf,

	/// URL of the server.
	#[structopt(long, short)]
	url: String,

	/// Directory to store the index repository.
	#[structopt(long)]
	#[structopt(default_value = "index")]
	index_dir: PathBuf,

	/// Directory to store added crates.
	#[structopt(long)]
	#[structopt(default_value = "crates")]
	crate_dir: PathBuf,

	/// Custom allowed registries for dependencies.
	#[structopt(long = "allowed-registry")]
	allowed_registries: Vec<String>,

	/// Do not automatically allow dependencies from crates.io.
	#[structopt(long)]
	no_crates_io: bool,
}

/// Add a crate to the registry.
#[derive(StructOpt)]
#[structopt(setting = AppSettings::ColoredHelp)]
#[structopt(setting = AppSettings::UnifiedHelpMessage)]
#[structopt(setting = AppSettings::DeriveDisplayOrder)]
struct AddCrate {
	/// The root of of registry to work on.
	#[structopt(long, short)]
	#[structopt(default_value = ".")]
	registry: PathBuf,

	/// The packaged crate file to add.
	crate_file: PathBuf,
}

/// Completely delete a crate from the registry.
#[derive(StructOpt)]
#[structopt(setting = AppSettings::ColoredHelp)]
#[structopt(setting = AppSettings::UnifiedHelpMessage)]
#[structopt(setting = AppSettings::DeriveDisplayOrder)]
struct DeleteCrate {
	/// The root of of registry to work on.
	#[structopt(long, short)]
	#[structopt(default_value = ".")]
	registry: PathBuf,

	/// The name of the crate to delete.
	name: String,
}

/// Yank a crate version from the registry.
#[derive(StructOpt)]
#[structopt(setting = AppSettings::ColoredHelp)]
#[structopt(setting = AppSettings::UnifiedHelpMessage)]
#[structopt(setting = AppSettings::DeriveDisplayOrder)]
struct YankCrate {
	/// The root of of registry to work on.
	#[structopt(long, short)]
	#[structopt(default_value = ".")]
	registry: PathBuf,

	/// The name of the crate to yank.
	name: String,

	/// The version to yank.
	version: String,
}

/// Unyank a crate version from the registry.
#[derive(StructOpt)]
#[structopt(setting = AppSettings::ColoredHelp)]
#[structopt(setting = AppSettings::UnifiedHelpMessage)]
#[structopt(setting = AppSettings::DeriveDisplayOrder)]
struct UnyankCrate {
	/// The root of of registry to work on.
	#[structopt(long, short)]
	#[structopt(default_value = ".")]
	registry: PathBuf,

	/// The name of the crate to yank.
	name: String,

	/// The version to yank.
	version: String,
}

fn main() {
	if do_main(Options::from_args()).is_err() {
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

	Registry::init(&command.registry, config)
		.map_err(|e| eprintln!("{}", e))
		.map(drop)
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
