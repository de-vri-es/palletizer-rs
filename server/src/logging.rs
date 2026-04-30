use std::io::Write;

/// Initialize the logging system with a pretty format.
///
/// Logging for the specified root module will be set to Error, Warn, Info, Debug or Trace, depending on the verbosity parameter.
/// Logging for all other modules is set to one level less verbose.
pub(crate) fn init(root_module: &str, extra_modules: &[&str], verbosity: i16) {
	let log_level = match verbosity {
		i16::MIN..=-2 => log::LevelFilter::Error,
		-1 => log::LevelFilter::Warn,
		0 => log::LevelFilter::Info,
		1 => log::LevelFilter::Debug,
		2..=i16::MAX => log::LevelFilter::Trace,
	};

	let extra_level = match verbosity {
		i16::MIN..=-1 => log::LevelFilter::Error,
		0 => log::LevelFilter::Warn,
		1 => log::LevelFilter::Info,
		2 => log::LevelFilter::Debug,
		3..=i16::MAX => log::LevelFilter::Trace,
	};

	let mut logger = env_logger::Builder::new();

	logger.format(
		move |buffer, record: &log::Record| {
			use env_logger::fmt::style::AnsiColor;
			use env_logger::fmt::style::Style;

			let now = chrono::Local::now();
			let mut level_style = Style::new().bold();

			let date_style = Style::new().fg_color(Some(AnsiColor::Cyan.into()));
			let time_style = Style::new().fg_color(Some(AnsiColor::Cyan.into()));

			let level;
			match record.level() {
				log::Level::Trace => {
					level = "Trace";
				},
				log::Level::Debug => {
					level = "Debug";
				},
				log::Level::Info => {
					level = "Info";
				},
				log::Level::Warn => {
					level = "Warn";
					level_style = level_style.fg_color(Some(AnsiColor::Yellow.into()));
				},
				log::Level::Error => {
					level = "Error";
					level_style = level_style.fg_color(Some(AnsiColor::Red.into()));
				},
			};

			writeln!(
				buffer,
				"[{date_style}{date}{date_style:#} {time_style}{time}{time_style:#}] {level_style}{level:<7}{level_style:#} {message}",
				date = now.format("%Y-%m-%d"),
				time = now.format("%H:%M:%S%.3f"),
				message = record.args(),
			)
		}
	);

	logger.filter_module(root_module, log_level);
	for module in extra_modules {
		logger.filter_module(module, extra_level);
	}

	logger.init();
}
