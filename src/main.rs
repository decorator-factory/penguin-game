#![expect(clippy::print_stdout, clippy::print_stderr)]

use std::{
    borrow::Cow,
    fs::File,
    io::Read,
    path::{
        Path,
        PathBuf,
    },
};

use macroquad::prelude::*;

mod compat;
mod demo;
mod draw_utils;
mod game;
mod input;
mod level_parsing;
mod levels;
mod raw_level;
mod text_utils;

#[cfg(target_family = "wasm")]
mod wasm;

fn main() {
    let conf = Conf {
        window_title: "Rocket Jumping Penguin".to_string(),
        window_width: 1280,
        window_height: 720,
        window_resizable: true,
        high_dpi: true,
        icon: None, // the icon takes up 22K in the binary (even on wasm where it has no effect)
        ..Default::default()
    };

    let args = cli::parse_args_or_die();

    macroquad::Window::from_config(conf, async move {
        fix_panic_handling();
        if let Err(e) = amain(args).await {
            eprintln!("{e}");
            std::process::exit(1);
        }
    });
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

async fn amain(args: Args) -> Result<(), String> {
    let (base_device, skip_until_update, last_update): (&mut dyn input::InputDevice, _, _) =
        match args.input_demo {
            Some(InputDemo { source, skip_until_update }) => {
                let in_movie = fetch_demo(source)?;
                let last_update = in_movie.last_update();
                (&mut demo::DemoPlayback::new(in_movie), skip_until_update, last_update)
            }
            None => (&mut input::MacroquadInput, 0, None),
        };

    if let Some(out_path) = args.record_to {
        with_exclusive_file(&out_path, async |out_file| {
            let threshold_upd = last_update.unwrap_or(0) + 1;
            let names = (Cow::Borrowed("playback"), Cow::Borrowed("recording"));
            let recorder = demo::DemoRecorder::new(input::MacroquadInput, threshold_upd);
            let mut composed =
                input::ComposedInput::new(base_device, recorder, threshold_upd, names);

            game::run_game(&mut composed, args.level_source, skip_until_update).await;

            let output_movie = composed.into_inner().1.collect_recording();
            demo::unparse_movie(&output_movie, out_file).map_err(|e| e.to_string())?;
            Ok(())
        })
        .await?;
        println!("Wrote {} successfully!", out_path.display());
    } else {
        game::run_game(base_device, args.level_source, skip_until_update).await;
    }

    Ok(())
}

fn fetch_demo(source: DemoSource) -> Result<demo::DemoMovie, String> {
    match source {
        DemoSource::Default => Ok(demo::make_default_demo_movie()),
        DemoSource::NewLevelDemo => Ok(demo::make_new_level_demo_movie()),
        DemoSource::File(path) => try_read_demo_movie(&path),
        DemoSource::Empty => Ok(demo::DemoMovie::default()),
    }
}

async fn with_exclusive_file(
    path: &Path,
    inner: impl AsyncFnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|e| format!("could not open output file {}: {}", path.display(), e))?;
    inner(&mut file).await?;

    // NOTE: the following will error if we're writing to /dev/null
    file.sync_all().map_err(|e| format!("while flushing the file: {e}"))
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
            "RJP panicked at {location} with message: {panic_message} and backtrace: {backtrace:?}\x00"
        );

        // SAFETY: string points to an explicitly 0-terminated string
        unsafe { wasm::penguin_set_panic_message(string.as_ptr()) };
    }));
}

#[derive(Default, Debug)]
struct Args {
    level_source: game::LevelSource,
    input_demo: Option<InputDemo>,
    record_to: Option<PathBuf>,
}

#[derive(Debug)]
enum DemoSource {
    Default,
    NewLevelDemo,
    Empty,
    #[cfg_attr(target_family = "wasm", expect(dead_code))]
    File(PathBuf),
}

#[derive(Debug)]
struct InputDemo {
    source: DemoSource,
    skip_until_update: u64,
}

impl clap::ValueEnum for game::LevelSource {
    fn value_variants<'a>() -> &'a [Self] {
        &[game::LevelSource::Default, game::LevelSource::New, game::LevelSource::Big]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new(match self {
            game::LevelSource::Default => "default",
            game::LevelSource::New => "new",
            game::LevelSource::Big => "big",
        }))
    }
}

mod cli {
    use super::game::LevelSource;
    use super::{
        Args,
        DemoSource,
        InputDemo,
    };

    #[cfg(not(target_family = "wasm"))]
    pub fn parse_args_or_die() -> Args {
        use std::ffi::OsString;
        use std::path::PathBuf;

        use clap::{
            Arg,
            Command,
            builder::{
                EnumValueParser,
                ValueParser,
            },
            value_parser,
        };

        let matches = Command::new("rjp")
            .version("v0.0")
            .about(
                "A 2D platformer game where you rocket jump as a penguin. \
                See the README at https://github.com/decorator-factory/rocket-jumping-penguin \
                for extended CLI help.")
            .propagate_version(true)
            .arg(Arg::new("write_demo")
                .short('w')
                .long("write-demo")
                .value_name("path")
                .value_parser(ValueParser::path_buf()))
            .arg(
                Arg::new("read_demo")
                .short('r')
                .long("read-demo")
                .value_name("@default|path")
                .value_parser(ValueParser::os_string()))
            .arg(
                Arg::new("level_id")
                .long("level-id")
                .value_name("level_id")
                .default_value("default")
                .value_parser(EnumValueParser::<LevelSource>::new())
            )
            .arg(Arg::new("skip_until_update")
                .long("skip-until-update")
                .help("When a demo is provided, the game will skip this many ticks, and then set the UPS to 1.")
                .short('u')
                .value_parser(value_parser!(u64))
                .default_value("0"))
            .get_matches();

        let level_source =
            matches.get_one::<LevelSource>("level_id").unwrap_or(&LevelSource::Default).clone();

        let demo_source = matches.get_one::<OsString>("read_demo").map(|path| {
            if path == "@default" {
                match level_source {
                    LevelSource::Default => DemoSource::Default,
                    LevelSource::New => DemoSource::NewLevelDemo,
                    LevelSource::Big => DemoSource::Empty,
                }
            } else {
                DemoSource::File(PathBuf::from(path))
            }
        });
        let skip_until_update = *matches.get_one::<u64>("skip_until_update").unwrap_or(&0);
        let input_demo = demo_source.map(|source| InputDemo { source, skip_until_update });
        let record_to = matches.get_one::<PathBuf>("write_demo").cloned();

        Args { level_source, input_demo, record_to }
    }

    #[cfg(target_family = "wasm")]
    pub fn parse_args_or_die() -> Args {
        use crate::wasm;

        let opts = wasm::read_options();
        macroquad::logging::info!("Read initialization options: {:?}", opts);

        let hash = opts.get("url_fragment").map_or("", String::as_ref);
        let is_demo = hash.contains("demo");
        let level_source = if hash.contains("new") {
            LevelSource::New
        } else if hash.contains("big") {
            LevelSource::Big
        } else {
            LevelSource::Default
        };
        let input_demo = is_demo
            .then_some(match level_source {
                LevelSource::Default => DemoSource::Default,
                LevelSource::New => DemoSource::NewLevelDemo,
                LevelSource::Big => DemoSource::Empty,
            })
            .map(|source| InputDemo { source, skip_until_update: 0 });

        Args { level_source, input_demo, record_to: None }
    }
}
