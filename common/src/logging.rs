use std::time::SystemTime;

use fern::colors::{Color, ColoredLevelConfig};

pub fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            let colors_line = ColoredLevelConfig::new()
                .info(Color::Green)
                .warn(Color::Yellow)
                .error(Color::Red);
            out.finish(format_args!(
                "{} [{} {} {}] {}",
                format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()),
                humantime::format_rfc3339_seconds(SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .level_for("fairy", log::LevelFilter::Trace)
        .level_for("fairy_common", log::LevelFilter::Trace)
        .chain(std::io::stdout())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}