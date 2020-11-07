use palletizer::Registry;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use structopt::clap::AppSettings;

#[derive(StructOpt)]
#[structopt(setting = AppSettings::ColoredHelp)]
#[structopt(setting = AppSettings::UnifiedHelpMessage)]
#[structopt(setting = AppSettings::DeriveDisplayOrder)]
struct Options {
	/// The root of of registry to work on.
	#[structopt(short = "C", long)]
	root: Option<PathBuf>,

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
	path: Option<PathBuf>,
}

/// Add a crate to the registry.
#[derive(StructOpt)]
struct AddCrate {
	/// The path of the crate.
	path: PathBuf,
}

/// Yank a crate version from the registry.
#[derive(StructOpt)]
struct YankCrate {
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
	let root = options.root.as_deref().unwrap_or(".".as_ref());
	match &options.command {
		Command::Init(command) => init(root, command),
		Command::Add(command) => add_crate(root, command),
		Command::Yank(command) => yank_crate(root, command),
	}
}

fn init(root: &Path, command: &Init) -> Result<(), ()> {
	let path = root.join(command.path.as_deref().unwrap_or(".".as_ref()));
	let config = palletizer::Config::example();
	Registry::init(path, &config)
		.map_err(|e| eprintln!("{}", e))
		.map(drop)
}

fn add_crate(root: &Path, command: &AddCrate) -> Result<(), ()> {
	let mut registry = Registry::open(root)
		.map_err(|e| eprintln!("{}", e))?;
	registry.add_crate(&command.path)
		.map_err(|e| eprintln!("{}", e))?;
	Ok(())
}

fn yank_crate(root: &Path, command: &YankCrate) -> Result<(), ()> {
	let mut registry = Registry::open(root)
		.map_err(|e| eprintln!("{}", e))?;
	registry.yank_crate(&command.name, &command.version)
		.map_err(|e| eprintln!("{}", e))?;
	Ok(())
}
