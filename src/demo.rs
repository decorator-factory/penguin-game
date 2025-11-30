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

impl core::fmt::Debug for DemoMovie {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DemoMovie").field("actions[]", &self.actions.len()).finish()
    }
}

impl DemoMovie {
    /// If `actions` is empty, adds an `at 0: look 0` action
    pub fn new(actions: Box<[(u64, DemoAction)]>) -> DemoMovie {
        {
            // ensure update numbers are non-decreasing
            let mut last_upd = 0u64;
            for (i, (upd, action)) in actions.iter().enumerate() {
                assert!(*upd >= last_upd, "Instruction out of place: {:?}", (i, upd, action));
                last_upd = *upd;
            }
        }

        DemoMovie { actions }
    }

    /// Last known update number recorded in the movie
    pub fn last_update(&self) -> Option<u64> {
        self.actions.last().map(|(upd, _)| *upd)
    }
}

#[derive(Debug)]
pub struct DemoPlayback {
    upd: u64,
    movie: DemoMovie,
    action_index: usize,
    look_angle: f32,
    inputs: EnumSet<Input>,
}

impl DemoPlayback {
    pub fn new(movie: DemoMovie) -> DemoPlayback {
        DemoPlayback { movie, upd: 0, action_index: 0, look_angle: 0.0, inputs: EnumSet::new() }
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

    fn next_update(&mut self) {
        if self.action_index >= self.movie.actions.len() {
            return;
        }

        loop {
            let Some((upd, action)) = self.movie.actions.get(self.action_index) else { return };

            if *upd == self.upd {
                self.handle_action(*action);
                self.action_index += 1;
            } else if *upd <= self.upd {
                debug_assert!(
                    false,
                    "At upd={upd}, action={action:?}: we're somehow past our next action"
                );
                self.upd = *upd;
                self.handle_action(*action);
            } else {
                self.upd += 1;
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
/// retrieve them as a [`DemoMovie`].
#[derive(Debug)]
pub struct DemoRecorder<D> {
    actions: Vec<(u64, DemoAction)>,
    current_inputs: EnumSet<Input>,
    wrapped: D,
    current_upd: u64,
    shot_cooldown: u64,
    look_cooldown: u64,
    last_recorded_look: f32,
}

impl<D: InputDevice> DemoRecorder<D> {
    pub fn new(wrapped: D, starting_update: u64) -> DemoRecorder<D> {
        DemoRecorder {
            actions: Vec::with_capacity(1024),
            current_inputs: EnumSet::new(),
            wrapped,
            current_upd: starting_update,
            shot_cooldown: 0,
            look_cooldown: 0,
            last_recorded_look: 0.0,
        }
    }

    pub fn collect_recording(self) -> DemoMovie {
        DemoMovie::new(self.actions.into_boxed_slice())
    }

    fn maybe_record_look_angle(&mut self) {
        let mut record_look_degrees: Option<f32> = None;
        let look_radians = self.wrapped.look_angle_radians();

        if self.wrapped.is_input_down(Input::Shoot) {
            if self.shot_cooldown == 0 {
                if (self.last_recorded_look - look_radians).abs() > 0.0001 {
                    record_look_degrees = Some(look_radians.to_degrees());
                    self.shot_cooldown = 5; // TODO: this is not a good solution. We need "rocket is shot" events and such
                    self.look_cooldown = 20;
                }
            } else {
                self.shot_cooldown -= 1;
            }
        } else {
            self.shot_cooldown = 0;
        }

        if self.look_cooldown == 0 {
            if (look_radians - self.last_recorded_look).abs() > 0.0001 {
                record_look_degrees = Some(look_radians.to_degrees());
                self.look_cooldown = 20;
            }
        } else {
            self.look_cooldown -= 1;
        }

        if let Some(deg) = record_look_degrees {
            self.last_recorded_look = look_radians;
            self.actions.push((self.current_upd, DemoAction::SetLookAngle(deg)));
        }
    }
}

impl<D: InputDevice> InputDevice for DemoRecorder<D> {
    fn next_update(&mut self) {
        self.wrapped.next_update();
        self.maybe_record_look_angle();

        // When recording a new update, consult the parent device to see
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

        for input in to_on {
            self.actions.push((self.current_upd, DemoAction::InputOn(input)));
        }
        for input in to_off {
            self.actions.push((self.current_upd, DemoAction::InputOff(input)));
        }

        self.current_inputs = (self.current_inputs | to_on) - to_off;

        self.current_upd += 1;
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
    lineno: usize,
    colno: usize,
    detail: DemoParseErrorDetail,
}

#[derive(thiserror::Error, PartialEq, Debug)]
pub enum DemoParseErrorDetail {
    #[error("Invalid demo movie header. Expected 'penguindemo-text-v0' and a newline")]
    InvalidHeader,
    #[error("{0}")]
    InvalidSyntax(&'static str),
    #[error("Update numbers in the demo movie must be in a non-decreasing order")]
    UpdateNumberDecreased,
}

fn expect_keyword<'src>(source: &'src [u8], prefix: &[u8]) -> Option<&'src [u8]> {
    source.starts_with(prefix).then(||
        // SAFETY: if source starts with prefix, it must be at least that long
        unsafe { source.get_unchecked(prefix.len()..) })
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
pub fn unparse_movie_to_string(movie: &DemoMovie) -> String {
    let mut buf = Vec::with_capacity(1 << 10);
    unparse_movie(movie, &mut buf).unwrap(); // Vec as std::io::Write never errors
    String::from_utf8(buf).unwrap() // so far movies are supposed to be UTF-8
}

pub fn unparse_movie(movie: &DemoMovie, w: &mut impl std::io::Write) -> std::io::Result<()> {
    fn format_input(inp: Input) -> &'static str {
        match inp {
            Input::Left => "left",
            Input::Right => "right",
            Input::Shoot => "shoot",
        }
    }

    writeln!(w, "penguindemo-text-v0")?;

    for (upd, action) in &movie.actions {
        write!(w, "at {upd}: ")?;
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

    let mut lineno = 1usize;
    let mut actions: Vec<(u64, DemoAction)> = Vec::with_capacity(1024);
    let mut last_upd = 0u64;
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
            let colno = 1 + line_start.len() - line.len();
            Err(DemoParseError { lineno, colno, detail })
        };

        let Some(line) = expect_keyword(line, b"at") else {
            return wrap_err(line, E::InvalidSyntax("Expected 'at' keyword"));
        };
        if !line.starts_with(b" ") {
            return wrap_err(line, E::InvalidSyntax("Expected space after 'at'"));
        }
        let line = line.trim_ascii_start();

        let upd_number_position = line; // saved for error reporting later
        let (digits, line) = split_while(line, u8::is_ascii_digit);
        if digits.is_empty() {
            return wrap_err(line, E::InvalidSyntax("expected decimal integer"));
        }

        // SAFETY: ASCII is valid UTF-8
        let digits = unsafe { str::from_utf8_unchecked(digits) };
        let Ok(upd) = digits.parse::<u64>() else {
            return wrap_err(line, E::InvalidSyntax("number is too large"));
        };

        if upd >= u64::MAX / 4 {
            // I don't think this is technically required, but update counts this high
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

        if upd < last_upd {
            return wrap_err(upd_number_position, E::UpdateNumberDecreased);
        }
        last_upd = upd;

        actions.push((upd, action));
    }

    Ok(DemoMovie::new(actions.into_boxed_slice()))
}

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
                Kw::Look => unreachable!(),
            };
            (line, action)
        }
        Kw::Look => {
            let (numeric, line) =
                split_while(line, |b| matches!(*b, b'0'..=b'9' | b'.' | b'-' | b'+'));
            if numeric.is_empty() {
                return Err((line, E::InvalidSyntax("expected decimal number")));
            }

            // SAFETY: ASCII is valid UTF-8
            let numeric = unsafe { str::from_utf8_unchecked(numeric) };
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

static DEFAULT_DEMO_SOURCE: &[u8] = include_bytes!("../demos/intended.demo");
static NEW_LEVEL_DEMO_SOURCE: &[u8] = include_bytes!("../demos/new_level.demo");

pub fn make_default_demo_movie() -> DemoMovie {
    parse_movie(DEFAULT_DEMO_SOURCE)
        .unwrap_or_else(|e| panic!("the demo movie in '../demos/intended.demo' is malformed: {e}"))
}

pub fn make_new_level_demo_movie() -> DemoMovie {
    parse_movie(NEW_LEVEL_DEMO_SOURCE)
        .unwrap_or_else(|e| panic!("the demo movie in '../demos/new_level.demo' is malformed: {e}"))
}

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
    pub fn split_while_empty() {
        let (left, right) = split_while(b"aaaabcd", |c| *c == b'?');
        assert_eq!((left, right), (&b""[..], &b"aaaabcd"[..]));
    }

    #[test]
    pub fn split_while_empty_string() {
        let (left, right) = split_while(b"", |c| *c == b'?');
        assert_eq!((left, right), (&b""[..], &b""[..]));
    }

    #[test]
    pub fn split_while_mixed() {
        let (left, right) = split_while(b"aaaabcd", |c| *c == b'a');
        assert_eq!((left, right), (&b"aaaa"[..], &b"bcd"[..]));
    }

    #[test]
    pub fn expect_keyword_ok() {
        assert_eq!(Some(&b" banana"[..]), expect_keyword(b"apple banana", b"apple"));
    }

    #[test]
    pub fn expect_keyword_fail() {
        assert_eq!(None, expect_keyword(b"apple banana", b"cherry"));
    }

    #[test]
    pub fn expect_keyword_fail_empty_source() {
        assert_eq!(None, expect_keyword(b"", b"cherry"));
    }

    #[test]
    pub fn wrong_version() {
        let source = b"penguindemo-text-v69\n";
        let err = parse_movie(source).unwrap_err();
        assert_eq!(err, DemoParseError {
            lineno: 1,
            colno: 1,
            detail: DemoParseErrorDetail::InvalidHeader
        });
    }

    #[test]
    pub fn wrong_header() {
        let source = b"\x00WRONG";
        let err = parse_movie(source).unwrap_err();
        assert_eq!(err, DemoParseError {
            lineno: 1,
            colno: 1,
            detail: DemoParseErrorDetail::InvalidHeader
        });
    }

    #[test]
    pub fn empty_movie() {
        let source = b"penguindemo-text-v0\n";
        let movie = parse_movie(source).unwrap();
        assert_eq!(movie.actions.as_ref(), []);
    }

    #[test]
    pub fn simple_movie() {
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
    pub fn look_movie() {
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
    pub fn movie_with_comments() {
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
    pub fn space_required_after_keyword() {
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
