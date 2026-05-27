use std::path::PathBuf;
use clap::Parser;
use fern::colors::{Color, ColoredLevelConfig};
use std::time::SystemTime;
use colored::Colorize;
use log::info;
use fuel::parse_file;

fn setup_logger() -> Result<(), fern::InitError> {
    let colors = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::White)
        .trace(Color::BrightBlack);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339_millis(SystemTime::now()).to_string().cyan(),
                colors.color(record.level()),
                record.target().blue(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

#[derive(Parser)]
struct Arguments {
    #[clap(short, long)]
    file: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger().expect("Failed to initialize logger");

    let args = Arguments::parse();
    info!("Processing file: {}", args.file.display());

    let file = parse_file(&args.file)?;
    info!("Successfully parsed file: {:?}", file);

    Ok(())
}
