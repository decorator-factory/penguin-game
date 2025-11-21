use std::borrow::Cow;

use crate::input::{
    Input,
    InputDevice,
};
use enumset::EnumSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DemoAction {
    InputOn(Input),
    InputOff(Input),
    SetLookAngle(f32), // degrees!
}

#[derive(Clone, PartialEq)]
pub struct DemoMovie {
    actions: Box<[(u64, DemoAction)]>,
}

impl std::fmt::Debug for DemoMovie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DemoMovie").field("actions[]", &self.actions.len()).finish()
    }
}

impl DemoMovie {
    /// If `actions` is empty, adds an `at 0: look 0` action
    pub fn new(actions: Box<[(u64, DemoAction)]>) -> DemoMovie {
        if actions.is_empty() {
            return DemoMovie::new(Box::new([(0, DemoAction::SetLookAngle(0.0))]));
        }

        {
            // ensure frame numbers are non-decreasing
            let mut last_frame = 0u64;
            for (i, (frame, action)) in actions.iter().enumerate() {
                assert!(*frame >= last_frame, "Instruction out of place: {:?}", (i, frame, action));
                last_frame = *frame;
            }
        }

        DemoMovie { actions }
    }

    /// Last known frame number recorded in the movie
    pub fn last_frame(&self) -> u64 {
        self.actions.last().unwrap().0
    }

    #[allow(dead_code)]
    pub fn actions(&self) -> &[(u64, DemoAction)] {
        &self.actions
    }
}

pub struct DemoPlayback {
    frame: u64,
    movie: DemoMovie,
    action_index: usize,
    look_angle: f32,
    inputs: EnumSet<Input>,
}

impl DemoPlayback {
    pub fn new(movie: DemoMovie) -> DemoPlayback {
        DemoPlayback { movie, frame: 0, action_index: 0, look_angle: 0.0, inputs: EnumSet::new() }
    }

    fn handle_action(&mut self, action: DemoAction) {
        match action {
            DemoAction::InputOn(input) => self.inputs |= input,
            DemoAction::InputOff(input) => self.inputs -= input,
            DemoAction::SetLookAngle(angle) => self.look_angle = angle.to_radians(),
        }
    }
}

impl InputDevice for DemoPlayback {
    fn is_input_down(&self, input: Input) -> bool {
        self.inputs.contains(input)
    }

    fn look_angle_radians(&self) -> f32 {
        self.look_angle
    }

    fn next_frame(&mut self) {
        self.frame += 1;

        if self.action_index >= self.movie.actions.len() {
            return;
        }

        loop {
            let Some((frame, action)) = self.movie.actions.get(self.action_index) else { return };
            if *frame == self.frame {
                self.handle_action(*action);
                self.action_index += 1;
            } else {
                return;
            }
        }
    }

    fn device_info(&'_ self) -> Cow<'_, str> {
        Cow::Borrowed("demo")
    }
}

// Demo recording

/// Decorator for an input device that records inputs
/// (in a smart way, to reduce the movie size) to eventually
/// retrieve them as a DemoMovie.
pub struct DemoRecorder<D> {
    actions: Vec<(u64, DemoAction)>,
    current_inputs: EnumSet<Input>,
    wrapped: D,
    current_frame: u64,
    shot_cooldown: u64,
}

impl<D> DemoRecorder<D> {
    pub fn new(wrapped: D, starting_frame: u64) -> DemoRecorder<D> {
        DemoRecorder {
            actions: Vec::with_capacity(1024),
            current_inputs: EnumSet::new(),
            wrapped,
            current_frame: starting_frame,
            shot_cooldown: 0,
        }
    }

    pub fn collect_recording(self) -> DemoMovie {
        DemoMovie::new(self.actions.into_boxed_slice())
    }
}

impl<D: InputDevice> InputDevice for DemoRecorder<D> {
    fn next_frame(&mut self) {
        // When recording a new frame, consult the parent device to see
        // what changed and potentially record demo commands
        let mut to_on = EnumSet::new();
        let mut to_off = EnumSet::new();

        for input in EnumSet::<Input>::all() {
            let wrapped_on = self.wrapped.is_input_down(input);

            if wrapped_on && !self.current_inputs.contains(input) {
                to_on |= input;
            }

            if !wrapped_on && self.current_inputs.contains(input) {
                to_off |= input;
            }
        }

        if to_on.contains(Input::Shoot) || self.current_inputs.contains(Input::Shoot) {
            if self.shot_cooldown == 0 {
                let radians = self.wrapped.look_angle_radians();
                self.actions
                    .push((self.current_frame, DemoAction::SetLookAngle(radians.to_degrees())));
                self.shot_cooldown = 16; // prevent spamming `look ...` when holding M1
            // TODO: how do we keep this in sync with actual rocket cooldown?
            } else {
                self.shot_cooldown -= 1;
            }
        } else {
            self.shot_cooldown = 0;
        }

        for input in to_on {
            self.actions.push((self.current_frame, DemoAction::InputOn(input)));
        }
        for input in to_off {
            self.actions.push((self.current_frame, DemoAction::InputOff(input)));
        }

        self.current_inputs = (self.current_inputs | to_on) - to_off;

        self.current_frame += 1;
    }

    fn is_input_down(&self, input: Input) -> bool {
        self.wrapped.is_input_down(input)
    }

    fn look_angle_radians(&self) -> f32 {
        self.wrapped.look_angle_radians()
    }

    fn device_info(&'_ self) -> Cow<'_, str> {
        Cow::Owned(format!("demo-recorder ({})", self.wrapped.device_info()))
    }
}

//-----

#[derive(thiserror::Error, PartialEq, Debug)]
#[error("{detail} at line {lineno}, column {colno}")]
pub struct DemoParseError {
    lineno: u32,
    colno: u32,
    detail: DemoParseErrorDetail,
}

#[derive(thiserror::Error, PartialEq, Debug)]
pub enum DemoParseErrorDetail {
    #[error("Invalid demo movie header. Expected 'penguindemo-text-v0' and a newline")]
    InvalidHeader,
    #[error("{0}")]
    InvalidSyntax(&'static str),
    #[error("Frame numbers in the demo movie must be in a non-decreasing order")]
    FrameDecreased,
}

fn expect_keyword<'src>(source: &'src [u8], prefix: &[u8]) -> Option<&'src [u8]> {
    if source.starts_with(prefix) {
        Some(unsafe { source.get_unchecked(prefix.len()..) })
    } else {
        None
    }
}

fn split_while<T, F>(slice: &[T], mut pred: F) -> (&[T], &[T])
where
    F: FnMut(&T) -> bool,
{
    if slice.is_empty() {
        return (slice, slice);
    }

    match slice.iter().position(|x| !pred(x)) {
        Some(index) => (&slice[..index], &slice[index..]),
        None => (slice, &slice[slice.len()..]),
    }
}

#[allow(dead_code)]
pub fn unparse_movie(movie: &DemoMovie, w: &mut impl std::io::Write) -> std::io::Result<()> {
    writeln!(w, "penguindemo-text-v0")?;

    fn format_input(inp: Input) -> &'static str {
        match inp {
            Input::Left => "left",
            Input::Right => "right",
            Input::Shoot => "shoot",
        }
    }

    for (frame, action) in &movie.actions {
        write!(w, "at {frame}: ")?;
        match action {
            DemoAction::InputOn(input) => writeln!(w, "on {}", format_input(*input))?,
            DemoAction::InputOff(input) => writeln!(w, "off {}", format_input(*input))?,
            DemoAction::SetLookAngle(angle) => writeln!(w, "look {angle}")?,
        }
    }

    Ok(())
}

pub fn parse_movie(source: &[u8]) -> Result<DemoMovie, DemoParseError> {
    // if this gets any more complicated, look into `nom` or other parsing libraries
    use DemoParseErrorDetail as E;

    let Some(source) = expect_keyword(source, b"penguindemo-text-v0\n") else {
        return Err(DemoParseError {
            lineno: 1,
            colno: 1,
            detail: DemoParseErrorDetail::InvalidHeader,
        });
    };

    let mut lineno: u32 = 1;
    let mut actions: Vec<(u64, DemoAction)> = Vec::with_capacity(1024);
    let mut last_frame = 0u64;
    for line in source.split(|b| *b == b'\n') {
        lineno += 1;
        let line_start = line;
        let line = line.trim_ascii();

        if line.is_empty() {
            continue;
        }

        if line.starts_with(b"#") {
            // comment
            continue;
        }

        let wrap_err = |line: &[u8], detail| {
            let colno = (1 + line_start.len() - line.len()) as u32;
            Err(DemoParseError { lineno, colno, detail })
        };

        let Some(line) = expect_keyword(line, b"at") else {
            return wrap_err(line, E::InvalidSyntax("Expected 'at' keyword"));
        };
        if !line.starts_with(b" ") {
            return wrap_err(line, E::InvalidSyntax("Expected space after 'at'"));
        }
        let line = line.trim_ascii_start();

        let frame_number_position = line; // saved for error reporting later
        let (digits, line) = split_while(line, |b| b.is_ascii_digit());
        if digits.is_empty() {
            return wrap_err(line, E::InvalidSyntax("expected decimal integer"));
        }

        let digits = unsafe { str::from_utf8_unchecked(digits) }; // SAFETY: ASCII is valid UTF-8
        let Ok(frame) = digits.parse::<u64>() else {
            return wrap_err(line, E::InvalidSyntax("number is too large"));
        };

        if frame >= u64::MAX / 4 {
            // I don't think this is technically required, but frame counts this high
            // are probably a typo or deliberately wrong input. So let's not panic further in
            // the code
            return wrap_err(line, E::InvalidSyntax("number is too large"));
        }

        let Some(line) = expect_keyword(line, b":") else {
            return wrap_err(line, E::InvalidSyntax("expected colon (:)"));
        };
        let line = line.trim_ascii_start();

        let (line, action) = match try_parse_action(line) {
            Ok((line, action)) => (line, action),
            Err((line, err)) => return wrap_err(line, err),
        };

        let line = line.trim_ascii_start();
        if !line.is_empty() && !line.starts_with(b"#") {
            // line doesn't end with a comment and is not empty -- something dangling
            return wrap_err(
                line,
                E::InvalidSyntax("unrecognized characters at the end of a line"),
            );
        }

        if frame < last_frame {
            return wrap_err(frame_number_position, E::FrameDecreased);
        }
        last_frame = frame;

        actions.push((frame, action));
    }

    Ok(DemoMovie::new(actions.into_boxed_slice()))
}

#[allow(clippy::type_complexity)]
fn try_parse_action(line: &[u8]) -> Result<(&[u8], DemoAction), (&[u8], DemoParseErrorDetail)> {
    use DemoParseErrorDetail as E;

    // line is already trimmed
    enum Kw {
        On,
        Off,
        Look,
    }

    let (kw, line) = if let Some(line) = expect_keyword(line, b"on") {
        (Kw::On, line)
    } else if let Some(line) = expect_keyword(line, b"off") {
        (Kw::Off, line)
    } else if let Some(line) = expect_keyword(line, b"look") {
        (Kw::Look, line)
    } else {
        return Err((line, E::InvalidSyntax("expected 'on', 'off', or 'look'")));
    };

    if !line.starts_with(b" ") {
        return Err((line, E::InvalidSyntax("Expected space after keyword")));
    }
    let line = line.trim_ascii_start();

    let (line, action) = match kw {
        Kw::On | Kw::Off => {
            let Some((line, input)) = try_parse_input(line) else {
                return Err((line, E::InvalidSyntax("expected 'left', 'right' or 'shoot'")));
            };
            let action = match kw {
                Kw::On => DemoAction::InputOn(input),
                Kw::Off => DemoAction::InputOff(input),
                _ => unreachable!(),
            };
            (line, action)
        }
        Kw::Look => {
            let (numeric, line) =
                split_while(line, |b| matches!(*b, b'0'..=b'9' | b'.' | b'-' | b'+'));
            if numeric.is_empty() {
                return Err((line, E::InvalidSyntax("expected decimal number")));
            }
            let numeric = unsafe { str::from_utf8_unchecked(numeric) }; // SAFETY: ASCII is valid UTF-8
            let Ok(angle) = str::parse::<f32>(numeric) else {
                return Err((line, E::InvalidSyntax("expected decimal number")));
            };
            (line, DemoAction::SetLookAngle(angle))
        }
    };

    Ok((line, action))
}

fn try_parse_input(line: &[u8]) -> Option<(&[u8], Input)> {
    if let Some(line) = expect_keyword(line, b"left") {
        Some((line, Input::Left))
    } else if let Some(line) = expect_keyword(line, b"right") {
        Some((line, Input::Right))
    } else if let Some(line) = expect_keyword(line, b"shoot") {
        Some((line, Input::Shoot))
    } else {
        None
    }
}

//-----

pub fn make_default_demo_movie() -> DemoMovie {
    parse_movie(DEFAULT_DEMO_SOURCE).unwrap()
}

static DEFAULT_DEMO_SOURCE: &[u8] = include_bytes!("../demos/intended.demo");

//-----

#[cfg(test)]
mod parse_tests {
    use crate::demo::{
        DemoAction,
        expect_keyword,
        split_while,
    };
    use crate::input::Input;

    use super::{
        DemoParseError,
        DemoParseErrorDetail,
        parse_movie,
    };

    #[test]
    pub fn test_split_while_empty() {
        let (left, right) = split_while(b"aaaabcd", |c| *c == b'?');
        assert_eq!((left, right), (&b""[..], &b"aaaabcd"[..]));
    }

    #[test]
    pub fn test_split_while_empty_string() {
        let (left, right) = split_while(b"", |c| *c == b'?');
        assert_eq!((left, right), (&b""[..], &b""[..]));
    }

    #[test]
    pub fn test_split_while_mixed() {
        let (left, right) = split_while(b"aaaabcd", |c| *c == b'a');
        assert_eq!((left, right), (&b"aaaa"[..], &b"bcd"[..]));
    }

    #[test]
    pub fn test_expect_ok() {
        assert_eq!(Some(&b" banana"[..]), expect_keyword(b"apple banana", b"apple"));
    }

    #[test]
    pub fn test_expect_fail() {
        assert_eq!(None, expect_keyword(b"apple banana", b"cherry"));
    }

    #[test]
    pub fn test_expect_fail_empty_source() {
        assert_eq!(None, expect_keyword(b"", b"cherry"));
    }

    #[test]
    pub fn test_wrong_version() {
        let source = b"penguindemo-text-v69\n";
        let err = parse_movie(source).unwrap_err();
        assert_eq!(err, DemoParseError {
            lineno: 1,
            colno: 1,
            detail: DemoParseErrorDetail::InvalidHeader,
        });
    }

    #[test]
    pub fn test_wrong_header() {
        let source = b"\x00WRONG";
        let err = parse_movie(source).unwrap_err();
        assert_eq!(err, DemoParseError {
            lineno: 1,
            colno: 1,
            detail: DemoParseErrorDetail::InvalidHeader,
        });
    }

    #[test]
    pub fn test_empty_movie() {
        let source = b"penguindemo-text-v0\n";
        let movie = parse_movie(source).unwrap();
        assert_eq!(movie.actions.as_ref(), [(0, DemoAction::SetLookAngle(0.0))]);
    }

    #[test]
    pub fn test_simple_movie() {
        let source = b"penguindemo-text-v0\n\
            at 0: on left\n\
            at 9: on shoot \n\
            at 100: off shoot\n\
            at 257: off left\n\
            at 1000:on right\n"; // space after : not required
        let movie = parse_movie(source).unwrap();
        assert_eq!(movie.actions.as_ref(), [
            (0, DemoAction::InputOn(Input::Left)),
            (9, DemoAction::InputOn(Input::Shoot)),
            (100, DemoAction::InputOff(Input::Shoot)),
            (257, DemoAction::InputOff(Input::Left)),
            (1000, DemoAction::InputOn(Input::Right)),
        ]);
    }

    #[test]
    pub fn test_look_movie() {
        let source = b"penguindemo-text-v0\n\
            at 42: look 90\n\
            at 69: look 0\n\
            at 420: look -0.12345\n";
        let movie = parse_movie(source).unwrap();
        assert_eq!(movie.actions.as_ref(), [
            (42, DemoAction::SetLookAngle(90.0)),
            (69, DemoAction::SetLookAngle(0.0)),
            (420, DemoAction::SetLookAngle(-0.12345)),
        ]);
    }

    #[test]
    pub fn test_movie_with_comments() {
        let source = b"penguindemo-text-v0\n\
            # this comment takes an entire line
            at 42: look 90  # this comment is after a line\n\
            at 69: on left  # this too\n\
            at 420: off shoot  # this too\n";
        let movie = parse_movie(source).unwrap();
        assert_eq!(movie.actions.as_ref(), [
            (42, DemoAction::SetLookAngle(90.0)),
            (69, DemoAction::InputOn(Input::Left)),
            (420, DemoAction::InputOff(Input::Shoot)),
        ]);
    }

    #[test]
    pub fn test_space_required_after_keyword() {
        let sources = [
            &b"penguindemo-text-v0\nat42: look 90\n"[..],
            &b"penguindemo-text-v0\nat 42: look90\n"[..],
            &b"penguindemo-text-v0\nat 42: onleft\n"[..],
        ];
        for source in sources {
            let err = parse_movie(source).unwrap_err();
            assert_eq!(err.lineno, 2);
            assert!(matches!(err.detail, DemoParseErrorDetail::InvalidSyntax(_)));
        }
    }
}
