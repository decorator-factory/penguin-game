#![expect(clippy::print_stdout, clippy::print_stderr)]

use std::{
    borrow::Cow,
    fs::File,
    io::Read,
    path::PathBuf,
};

use macroquad::prelude::*;

use crate::game::LevelSource;

mod demo;
mod draw_utils;
mod game;
mod generated_levels;
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
    let args = RunArgs::from_cli_args(args);

    macroquad::Window::from_config(conf, async move {
        fix_panic_handling();
        if let Err(e) = amain(args).await {
            eprintln!("{e}");
            std::process::exit(1);
        }
    });
}

#[derive(Debug)]
enum DemoSource {
    Default,
    NewLevelDemo,
    File(PathBuf),
}

#[derive(Default, Debug)]
struct RunArgs {
    level_source: LevelSource,
    input_demo: Option<(DemoSource, u64)>,
    record_to: Option<PathBuf>,
    keep_playing: bool,
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

impl RunArgs {
    fn from_cli_args(args: cli::Args) -> RunArgs {
        match args {
            cli::Args::JustPlay { is_new_level } => RunArgs {
                level_source: if is_new_level { LevelSource::New } else { LevelSource::Default },
                ..Default::default()
            },
            cli::Args::PlayDemo { input_path, skip_until_update } => {
                let source = match input_path {
                    Some(path) => DemoSource::File(path),
                    None => DemoSource::Default,
                };
                RunArgs { input_demo: Some((source, skip_until_update)), ..Default::default() }
            }
            cli::Args::RecordDemo { output_path } => RunArgs {
                input_demo: None,
                level_source: LevelSource::Default,
                record_to: Some(output_path),
                ..Default::default()
            },
            cli::Args::ReRecordDemo { input_path, output_path } => RunArgs {
                input_demo: Some((DemoSource::File(input_path), 0)),
                level_source: LevelSource::Default,
                record_to: Some(output_path),
                ..Default::default()
            },
            cli::Args::KeepRecordingDemo { input_path, output_path } => RunArgs {
                input_demo: Some((DemoSource::File(input_path), 0)),
                level_source: LevelSource::Default,
                record_to: Some(output_path),
                keep_playing: true,
            },
            cli::Args::PlayNewLevelDemo => RunArgs {
                input_demo: Some((DemoSource::NewLevelDemo, 0)),
                level_source: LevelSource::New,
                ..Default::default()
            },
        }
    }
}

async fn amain(args: RunArgs) -> Result<(), String> {
    let in_movie;
    let mut demo_playback;
    let (base_device, skip_until_update, last_update): (&mut dyn input::InputDevice, _, _) =
        match args.input_demo {
            Some((source, skip_updates)) => {
                in_movie = match source {
                    DemoSource::Default => demo::make_default_demo_movie(),
                    DemoSource::NewLevelDemo => demo::make_new_level_demo_movie(),
                    DemoSource::File(path) => try_read_demo_movie(&path)?,
                };
                let last_update = in_movie.last_update();
                demo_playback = demo::DemoPlayback::new(in_movie);
                (&mut demo_playback, skip_updates, last_update)
            }
            None => (&mut input::MacroquadInput, 0, None),
        };

    if let Some(out_path) = args.record_to {
        let mut out_file = try_create_exclusive_file(&out_path)?;

        let output_movie = if args.keep_playing {
            let threshold_upd = last_update.unwrap_or(0) + 1;
            let names = (Cow::Borrowed("playback"), Cow::Borrowed("recording"));
            let recorder = demo::DemoRecorder::new(input::MacroquadInput, threshold_upd);
            let mut composed =
                input::ComposedInput::new(base_device, recorder, threshold_upd, names);
            game::run_game(&mut composed, args.level_source, skip_until_update).await;
            composed.into_inner().1.collect_recording()
        } else {
            let mut recorder = demo::DemoRecorder::new(input::MacroquadInput, 0);
            game::run_game(&mut recorder, args.level_source, skip_until_update).await;
            recorder.collect_recording()
        };

        let write_result =
            demo::unparse_movie(&output_movie, &mut out_file).and_then(|()| out_file.sync_all());

        if let Err(e) = write_result {
            return Err(format!("Failed to write demo movie to {}: {}", out_path.display(), e));
        }
        println!("Wrote {} successfully!", out_path.display());
    } else {
        game::run_game(base_device, args.level_source, skip_until_update).await;
    }

    Ok(())
}

/// Improve panic handling on webassembly.
///
/// See <https://github.com/not-fl3/macroquad/issues/953> for some context.
///
/// Additionally, macroquad's default panic hook just shows `Any {..}`
/// for the payload which makes debugging difficult.
fn fix_panic_handling() {
    #[cfg(target_family = "wasm")]
    std::panic::set_hook(Box::new(|panic_info| {
        let panic_message = panic_info.payload_as_str().unwrap_or("<unknown payload>");
        let location =
            panic_info.location().map_or("<unknown location>".to_string(), ToString::to_string);

        let backtrace = std::backtrace::Backtrace::force_capture();
        let string = format!(
            "penguin-game panicked at {location} with message: {panic_message} and backtrace: {backtrace:?}\x00"
        );

        // SAFETY: string points to an explicitly 0-terminated string
        unsafe {
            wasm::set_panic_message(string.as_ptr().cast());
        };
    }));
}

mod cli {
    use std::path::PathBuf;

    #[cfg(not(target_family = "wasm"))]
    use clap::{
        Arg,
        ArgAction,
        Command,
        builder::ValueParser,
    };

    #[allow(dead_code)]
    #[derive(Debug)]
    pub enum Args {
        JustPlay { is_new_level: bool },
        PlayDemo { input_path: Option<PathBuf>, skip_until_update: u64 },
        PlayNewLevelDemo, // wasm only -- honestly it's all one big hack
        RecordDemo { output_path: PathBuf },
        ReRecordDemo { input_path: PathBuf, output_path: PathBuf },
        KeepRecordingDemo { input_path: PathBuf, output_path: PathBuf },
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn parse_args_or_die() -> Args {
        use clap::value_parser;

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
            .subcommand(
                Command::new("play")
                    .about("Play the game normally")
                    .arg(Arg::new("new_level").long("new-level").action(ArgAction::SetTrue)),
            )
            .subcommand(
                Command::new("demo")
                    .about("Play back a demo movie")
                    .arg(input_file_arg().required(false))
                    .arg(
                        Arg::new("skip_until_update")
                            .long("skip-until-update")
                            .help("if present, run movie very fast until this update number and then resume at UPS=1")
                            .short('u')
                            .value_parser(value_parser!(u64))
                            .default_value("0"),
                    ),
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
            ("play", args) => Args::JustPlay { is_new_level: *args.get_one("new_level").unwrap() },
            ("demo", args) => Args::PlayDemo {
                input_path: args.get_one("input_file").cloned(),
                skip_until_update: *args.get_one("skip_until_update").unwrap(),
            },
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
            Args::PlayDemo { input_path: None, skip_until_update: 0 }
        } else if crate::wasm::is_wasm_new_level_demo() {
            Args::PlayNewLevelDemo
        } else {
            Args::JustPlay { is_new_level: crate::wasm::is_wasm_new_level() }
        }
    }
}
