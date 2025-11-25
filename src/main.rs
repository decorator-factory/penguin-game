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

    let args = cli::parse_args_or_die();
    let args = RunArgs::from_cli_args(args).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });

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
    fn from_cli_args(args: cli::Args) -> Result<RunArgs, String> {
        match args {
            cli::Args::JustPlay => Ok(RunArgs::JustPlay),
            cli::Args::PlayDemo { input_path } => {
                let in_movie = match input_path {
                    Some(path) => try_read_demo_movie(&path)?,
                    None => demo::make_default_demo_movie(),
                };
                Ok(RunArgs::PlayDemo { in_movie })
            }
            cli::Args::RecordDemo { output_path } => {
                let output_file = try_create_exclusive_file(&output_path)?;
                Ok(RunArgs::RecordDemo { output_file, output_path })
            }
            cli::Args::ReRecordDemo { input_path, output_path } => {
                let in_movie = try_read_demo_movie(&input_path)?;
                let output_file = try_create_exclusive_file(&output_path)?;
                Ok(RunArgs::ReRecordDemo { in_movie, output_file, output_path })
            }
            cli::Args::KeepRecordingDemo { input_path, output_path } => {
                let in_movie = try_read_demo_movie(&input_path)?;
                let output_file = try_create_exclusive_file(&output_path)?;
                Ok(RunArgs::KeepRecordingDemo { in_movie, output_file, output_path })
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
                let threshold_upd = in_movie.last_update().unwrap_or(0) + 1;
                let playback = demo::DemoPlayback::new(in_movie);
                let recorder = demo::DemoRecorder::new(input::MacroquadInput, threshold_upd);
                let names = (Cow::Borrowed("playback"), Cow::Borrowed("recording"));
                input::ComposedInput::new(playback, recorder, threshold_upd, names)
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

    #[cfg(not(target_family = "wasm"))]
    use clap::{
        Arg,
        Command,
        builder::ValueParser,
    };

    #[cfg_attr(target_family = "wasm", allow(dead_code))]
    #[derive(Debug)]
    pub enum Args {
        JustPlay,
        PlayDemo { input_path: Option<PathBuf> },
        RecordDemo { output_path: PathBuf },
        ReRecordDemo { input_path: PathBuf, output_path: PathBuf },
        KeepRecordingDemo { input_path: PathBuf, output_path: PathBuf },
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn parse_args_or_die() -> Args {
        let output_file_arg = || {
            Arg::new("output_file")
                .short('o')
                .long("output-file")
                .value_name("path")
                .required(true)
                .value_parser(ValueParser::path_buf())
        };
        let input_file_arg = || {
            Arg::new("input_file")
                .short('i')
                .long("input-file")
                .value_name("path")
                .required(true)
                .value_parser(ValueParser::path_buf())
        };
        let matches = Command::new("penguin-game")
            .version("v0.0")
            .about(
                "A 2D platformer game where you rocket jump as a penguin. \
                See the README at https://github.com/decorator-factory/penguin-game \
                for extended CLI help.",
            )
            .propagate_version(true)
            .subcommand_required(true)
            .arg_required_else_help(true)
            .subcommand(Command::new("play").about("Play the game normally"))
            .subcommand(
                Command::new("demo")
                    .about("Play back a demo movie")
                    .arg(input_file_arg().required(false)),
            )
            .subcommand(
                Command::new("record-demo")
                    .about("Record a demo movie and save it to a file when the game is closed")
                    .arg(output_file_arg()),
            )
            .subcommand(
                Command::new("keep-recording-demo")
                    .about(
                        "Play a demo from `input-file`, then record a demo movie fragment \
                            and save it to `output-file`",
                    )
                    .arg(input_file_arg())
                    .arg(output_file_arg()),
            )
            .subcommand(
                Command::new("re-record-demo")
                    .about(
                        "Play back a demo from `input-file` and also record it, \
                        saving it to `output-file`",
                    )
                    .arg(input_file_arg())
                    .arg(output_file_arg()),
            )
            .get_matches();

        match matches.subcommand().unwrap() {
            ("play", _) => Args::JustPlay,
            ("demo", args) => Args::PlayDemo { input_path: args.get_one("input_file").cloned() },
            ("record-demo", args) => Args::RecordDemo {
                output_path: args.get_one::<PathBuf>("output_file").unwrap().clone(),
            },
            ("keep-recording-demo", args) => Args::KeepRecordingDemo {
                input_path: args.get_one::<PathBuf>("input_file").unwrap().clone(),
                output_path: args.get_one::<PathBuf>("output_file").unwrap().clone(),
            },
            ("re-record-demo", args) => Args::ReRecordDemo {
                input_path: args.get_one::<PathBuf>("input_file").unwrap().clone(),
                output_path: args.get_one::<PathBuf>("output_file").unwrap().clone(),
            },
            _ => unreachable!(),
        }
    }

    #[cfg(target_family = "wasm")]
    pub fn parse_args_or_die() -> Args {
        if crate::wasm::is_wasm_demo() {
            Args::PlayDemo { input_path: None }
        } else {
            Args::JustPlay
        }
    }
}
