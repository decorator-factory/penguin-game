#![expect(clippy::print_stdout, clippy::print_stderr)]

use std::{
    borrow::Cow,
    fs::File,
    io::Read,
    path::PathBuf,
};

use macroquad::prelude::*;

mod demo;
mod draw_utils;
mod game;
mod input;
mod levels;
#[cfg(not(target_family = "wasm"))]
mod pico_args;
mod wasm;

fn main() {
    let conf = Conf {
        window_title: "penguin".to_string(),
        window_width: 1280,
        window_height: 720,
        window_resizable: true,
        sample_count: 2,
        high_dpi: true,
        ..Default::default()
    };

    let args = match cli::parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Invalid command-line arguments: {e}");
            std::process::exit(1);
        }
    };

    let args = match RunArgs::from_cli_args(args) {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!("{}", cli::HELP);
            return;
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    macroquad::Window::from_config(conf, amain(args));
}

enum RunArgs {
    JustPlay,
    PlayDemo { in_movie: demo::DemoMovie },
    RecordDemo { output_file: File, output_path: PathBuf },
    ReRecordDemo { in_movie: demo::DemoMovie, output_file: File, output_path: PathBuf },
    KeepRecordingDemo { in_movie: demo::DemoMovie, output_file: File, output_path: PathBuf },
}

impl RunArgs {
    fn from_cli_args(args: cli::Args) -> Result<Option<RunArgs>, String> {
        match args {
            cli::Args::ShowHelp => Ok(None),
            cli::Args::JustPlay => Ok(Some(RunArgs::JustPlay)),
            cli::Args::PlayDemo { input_path } => {
                let in_movie = match input_path {
                    Some(path) => try_read_demo_movie(&path)?,
                    None => demo::make_default_demo_movie(),
                };
                Ok(Some(RunArgs::PlayDemo { in_movie }))
            }
            cli::Args::RecordDemo { output_path } => {
                let output_file = try_create_exclusive_file(&output_path)?;
                Ok(Some(RunArgs::RecordDemo { output_file, output_path }))
            }
            cli::Args::ReRecordDemo { input_path, output_path } => {
                let in_movie = try_read_demo_movie(&input_path)?;
                let output_file = try_create_exclusive_file(&output_path)?;
                Ok(Some(RunArgs::ReRecordDemo { in_movie, output_file, output_path }))
            }
            cli::Args::KeepRecordingDemo { input_path, output_path } => {
                let in_movie = try_read_demo_movie(&input_path)?;
                let output_file = try_create_exclusive_file(&output_path)?;
                Ok(Some(RunArgs::KeepRecordingDemo { in_movie, output_file, output_path }))
            }
        }
    }
}

fn try_read_demo_movie(input_path: &std::path::Path) -> Result<demo::DemoMovie, String> {
    let mut input_file = File::open(input_path)
        .map_err(|e| format!("Could not open input file {}: {}", input_path.display(), e))?;

    let mut buf = Vec::with_capacity(1 << 20);
    if let Err(e) = input_file.read_to_end(&mut buf) {
        return Err(format!("Could not read input file {}: {}", input_path.display(), e));
    }
    demo::parse_movie(buf.as_ref())
        .map_err(|e| format!("Problem in demo file {}: {}", input_path.display(), e))
}

fn try_create_exclusive_file(output_path: &std::path::Path) -> Result<File, String> {
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(output_path);
    file.map_err(|e| format!("Could not open output file {}: {}", output_path.display(), e))
}

async fn amain(args: RunArgs) {
    match args {
        RunArgs::JustPlay => {
            game::run_game(&mut input::MacroquadInput).await;
        }
        RunArgs::PlayDemo { in_movie } => {
            let mut device = demo::DemoPlayback::new(in_movie);
            game::run_game(&mut device).await;
        }
        RunArgs::RecordDemo { mut output_file, output_path } => {
            let mut device = demo::DemoRecorder::new(input::MacroquadInput, 0);
            game::run_game(&mut device).await;
            let movie = device.collect_recording();

            if let Err(e) = demo::unparse_movie(&movie, &mut output_file) {
                eprintln!("Failed to write demo movie to {}: {}", output_path.display(), e);
                std::process::exit(1);
            }
            #[cfg_attr(target_family = "wasm", allow(clippy::drop_non_drop))]
            drop(output_file);
            println!("Wrote {} successfully!", output_path.display());
        }
        RunArgs::ReRecordDemo { in_movie, mut output_file, output_path } => {
            let mut device = demo::DemoRecorder::new(demo::DemoPlayback::new(in_movie), 0);
            game::run_game(&mut device).await;
            let output_movie = device.collect_recording();

            if let Err(e) = demo::unparse_movie(&output_movie, &mut output_file) {
                eprintln!("Failed to write demo movie to {}: {}", output_path.display(), e);
                std::process::exit(1);
            }
            #[cfg_attr(target_family = "wasm", allow(clippy::drop_non_drop))]
            drop(output_file);
            println!("Wrote {} successfully!", output_path.display());
        }
        RunArgs::KeepRecordingDemo { in_movie, mut output_file, output_path } => {
            let mut device = {
                let threshold_frame = in_movie.last_frame().unwrap_or(0) + 1;
                let playback = demo::DemoPlayback::new(in_movie);
                let recorder = demo::DemoRecorder::new(input::MacroquadInput, threshold_frame);
                let names = (Cow::Borrowed("playback"), Cow::Borrowed("recording"));
                input::ComposedInput::new(playback, recorder, threshold_frame, names)
            };
            game::run_game(&mut device).await;
            let output_movie = device.into_inner().1.collect_recording();

            if let Err(e) = demo::unparse_movie(&output_movie, &mut output_file) {
                eprintln!("Failed to write demo movie to {}: {}", output_path.display(), e);
                std::process::exit(1);
            }
            #[cfg_attr(target_family = "wasm", allow(clippy::drop_non_drop))]
            drop(output_file);
            println!("Wrote {} successfully!", output_path.display());
        }
    }
}

mod cli {
    use std::path::PathBuf;

    pub const HELP: &str = "\
penguin-game

Usage:
    penguin-game
    penguin-game demo [--input-file <path>]
    penguin-game record-demo --output-file <path>
    penguin-game keep-recording-demo --input-file <path> --output-file <path>
    penguin-game re-record-demo --input-file <path> --output-file <path>

See the README of the project for more details.";

    #[cfg_attr(target_family = "wasm", allow(dead_code))]
    #[derive(Debug)]
    pub enum Args {
        ShowHelp,
        JustPlay,
        PlayDemo { input_path: Option<PathBuf> },
        RecordDemo { output_path: PathBuf },
        ReRecordDemo { input_path: PathBuf, output_path: PathBuf },
        KeepRecordingDemo { input_path: PathBuf, output_path: PathBuf },
    }

    #[cfg(not(target_family = "wasm"))]
    #[derive(thiserror::Error, Debug)]
    pub enum CliError {
        #[error("Expected a subcommand")]
        ExpectedSubcommand,
        #[error("Unknown subcommand: {0}")]
        UnknownSubcommand(String),
        #[error("{0}")]
        PicoArgs(#[from] crate::pico_args::PicoError),
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn parse_args() -> Result<Args, CliError> {
        #[allow(clippy::unnecessary_wraps)]
        fn parse_path(s: &std::ffi::OsStr) -> Result<PathBuf, &'static str> {
            Ok(s.into())
        }

        let mut pargs = crate::pico_args::Arguments::from_env();

        if pargs.contains(["-h", "--help"]) {
            return Ok(Args::ShowHelp);
        }

        let Some(subcommand) = pargs.subcommand()? else {
            if pargs.0.is_empty() {
                return Ok(Args::JustPlay);
            } else {
                return Err(CliError::ExpectedSubcommand);
            }
        };

        match subcommand.as_ref() {
            "play" => Ok(Args::JustPlay),
            "demo" => {
                let input_path = pargs.opt_value_from_os_str("--input-file", parse_path)?;
                Ok(Args::PlayDemo { input_path })
            }
            "record-demo" => {
                let output_path = pargs.value_from_os_str("--output-file", parse_path)?;
                Ok(Args::RecordDemo { output_path })
            }
            "re-record-demo" => {
                let input_path = pargs.value_from_os_str("--input-file", parse_path)?;
                let output_path = pargs.value_from_os_str("--output-file", parse_path)?;
                Ok(Args::ReRecordDemo { input_path, output_path })
            }
            "keep-recording-demo" => {
                let input_path = pargs.value_from_os_str("--input-file", parse_path)?;
                let output_path = pargs.value_from_os_str("--output-file", parse_path)?;
                Ok(Args::KeepRecordingDemo { input_path, output_path })
            }
            s => Err(CliError::UnknownSubcommand(s.to_string())),
        }
    }

    #[cfg(target_family = "wasm")]
    #[allow(clippy::unnecessary_wraps)]
    pub fn parse_args() -> Result<Args, &'static str> {
        Ok(if crate::wasm::is_wasm_demo() {
            Args::PlayDemo { input_path: None }
        } else {
            Args::JustPlay
        })
    }
}
