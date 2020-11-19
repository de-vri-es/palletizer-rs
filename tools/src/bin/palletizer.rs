use palletizer::Registry;
use std::path::{Path, PathBuf};
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
	Yank(YankCrate),
}

/// Initialize a new registry.
#[derive(StructOpt)]
struct Init {
	/// The path of the registry to initialize.
	registry: Option<PathBuf>,
}

/// Add a crate to the registry.
#[derive(StructOpt)]
struct AddCrate {
	/// The root of of registry to work on.
	#[structopt(long, short)]
	registry: Option<PathBuf>,

	/// The packaged crate file to add.
	crate_file: PathBuf,
}

/// Yank a crate version from the registry.
#[derive(StructOpt)]
struct YankCrate {
	/// The root of of registry to work on.
	#[structopt(long, short)]
	registry: Option<PathBuf>,

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
		Command::Yank(command) => yank_crate(command),
	}
}

fn init(command: &Init) -> Result<(), ()> {
	let registry = command.registry.as_deref().unwrap_or(".".as_ref());
	let config = palletizer::Config::example();
	Registry::init(registry, config)
		.map_err(|e| eprintln!("{}", e))
		.map(drop)
}

fn add_crate(command: &AddCrate) -> Result<(), ()> {
	let registry = command.registry.as_deref().unwrap_or(".".as_ref());
	let mut registry = Registry::open(registry)
		.map_err(|e| eprintln!("{}", e))?;
	registry.add_crate_from_file(&command.crate_file)
		.map_err(|e| eprintln!("{}", e))?;
	Ok(())
}

fn yank_crate(command: &YankCrate) -> Result<(), ()> {
	let registry = command.registry.as_deref().unwrap_or(".".as_ref());
	let mut registry = Registry::open(registry)
		.map_err(|e| eprintln!("{}", e))?;
	registry.yank_crate(&command.name, &command.version)
		.map_err(|e| eprintln!("{}", e))?;
	Ok(())
}
