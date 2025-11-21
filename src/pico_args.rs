#![allow(dead_code)]
/*
https://github.com/RazrFalcon/pico-args/blob/ee9e334fdd5bd5dac16ec882ec4bced1d6c11bc9/src/lib.rs
This file was auto-formatted, some clippy nitpicks were fixed, and all the configuration flags were removed.

Original license:

Copyright (c) 2019 Yevhenii Reizner

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
*/
/*!
An ultra simple CLI arguments parser.

If you think that this library doesn't support some feature, it's probably intentional.

- No help generation
- Only flags, options, free arguments and subcommands are supported
- Arguments can be in any order
- Non UTF-8 arguments are supported
*/

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::ffi::{
    OsStr,
    OsString,
};
use std::fmt::{
    self,
    Display,
};
use std::str::FromStr;

/// A list of possible errors.
#[derive(Clone, Debug)]
pub enum Error {
    /// Arguments must be a valid UTF-8 strings.
    NonUtf8Argument,

    /// A missing free-standing argument.
    MissingArgument,

    /// A missing option.
    MissingOption(Keys),

    /// An option without a value.
    OptionWithoutAValue(&'static str),

    /// Failed to parse a UTF-8 free-standing argument.
    #[allow(missing_docs)]
    Utf8ArgumentParsingFailed { value: String, cause: String },

    /// Failed to parse a raw free-standing argument.
    #[allow(missing_docs)]
    ArgumentParsingFailed { cause: String },
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NonUtf8Argument => {
                write!(f, "argument is not a UTF-8 string")
            }
            Error::MissingArgument => {
                write!(f, "free-standing argument is missing")
            }
            Error::MissingOption(key) => {
                if key.second().is_empty() {
                    write!(f, "the '{}' option must be set", key.first())
                } else {
                    write!(f, "the '{}/{}' option must be set", key.first(), key.second())
                }
            }
            Error::OptionWithoutAValue(key) => {
                write!(f, "the '{}' option doesn't have an associated value", key)
            }
            Error::Utf8ArgumentParsingFailed { value, cause } => {
                write!(f, "failed to parse '{}': {}", value, cause)
            }
            Error::ArgumentParsingFailed { cause } => {
                write!(f, "failed to parse a binary argument: {}", cause)
            }
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, PartialEq)]
enum PairKind {
    TwoArguments,
}

/// An arguments parser.
#[derive(Clone, Debug)]
pub struct Arguments(pub Vec<OsString>);

impl Arguments {
    /// Creates a parser from a vector of arguments.
    ///
    /// The executable path **must** be removed.
    ///
    /// This can be used for supporting `--` arguments to forward to another program.
    /// See `examples/dash_dash.rs` for an example.
    pub fn from_vec(args: Vec<OsString>) -> Self {
        Arguments(args)
    }

    /// Creates a parser from [`env::args_os`].
    ///
    /// The executable path will be removed.
    ///
    /// [`env::args_os`]: https://doc.rust-lang.org/stable/std/env/fn.args_os.html
    pub fn from_env() -> Self {
        let mut args: Vec<_> = std::env::args_os().collect();
        args.remove(0);
        Arguments(args)
    }

    /// Parses the name of the subcommand, that is, the first positional argument.
    ///
    /// Returns `None` when subcommand starts with `-` or when there are no arguments left.
    ///
    /// # Errors
    ///
    /// - When arguments is not a UTF-8 string.
    pub fn subcommand(&mut self) -> Result<Option<String>, Error> {
        if self.0.is_empty() {
            return Ok(None);
        }

        if let Some(s) = self.0[0].to_str()
            && s.starts_with('-')
        {
            return Ok(None);
        }

        self.0.remove(0).into_string().map_err(|_| Error::NonUtf8Argument).map(Some)
    }

    /// Checks that arguments contain a specified flag.
    ///
    /// Searches through all arguments, not only the first/next one.
    ///
    /// Calling this method "consumes" the flag: if a flag is present `n`
    /// times then the first `n` calls to `contains` for that flag will
    /// return `true`, and subsequent calls will return `false`.
    pub fn contains<A: Into<Keys>>(&mut self, keys: A) -> bool {
        self.contains_impl(keys.into())
    }

    #[inline(never)]
    fn contains_impl(&mut self, keys: Keys) -> bool {
        if let Some((idx, _)) = self.index_of(keys) {
            self.0.remove(idx);
            true
        } else {
            false
        }
    }

    /// Parses a key-value pair using `FromStr` trait.
    ///
    /// This is a shorthand for `value_from_fn("--key", FromStr::from_str)`
    pub fn value_from_str<A, T>(&mut self, keys: A) -> Result<T, Error>
    where
        A: Into<Keys>,
        T: FromStr,
        <T as FromStr>::Err: Display,
    {
        self.value_from_fn(keys, FromStr::from_str)
    }

    /// Parses a key-value pair using a specified function.
    ///
    /// Searches through all argument, not only the first/next one.
    ///
    /// When a key-value pair is separated by a space, the algorithm
    /// will threat the next argument after the key as a value,
    /// even if it has a `-/--` prefix.
    /// So a key-value pair like `--key --value` is not an error.
    ///
    /// Must be used only once for each option.
    ///
    /// # Errors
    ///
    /// - When option is not present.
    /// - When key or value is not a UTF-8 string. Use [`value_from_os_str`] instead.
    /// - When value parsing failed.
    /// - When key-value pair is separated not by space or `=`.
    ///
    /// [`value_from_os_str`]: struct.Arguments.html#method.value_from_os_str
    pub fn value_from_fn<A: Into<Keys>, T, E: Display>(
        &mut self,
        keys: A,
        f: fn(&str) -> Result<T, E>,
    ) -> Result<T, Error> {
        let keys = keys.into();
        match self.opt_value_from_fn(keys, f) {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Err(Error::MissingOption(keys)),
            Err(e) => Err(e),
        }
    }

    /// Parses an optional key-value pair using `FromStr` trait.
    ///
    /// This is a shorthand for `opt_value_from_fn("--key", FromStr::from_str)`
    pub fn opt_value_from_str<A, T>(&mut self, keys: A) -> Result<Option<T>, Error>
    where
        A: Into<Keys>,
        T: FromStr,
        <T as FromStr>::Err: Display,
    {
        self.opt_value_from_fn(keys, FromStr::from_str)
    }

    /// Parses an optional key-value pair using a specified function.
    ///
    /// The same as [`value_from_fn`], but returns `Ok(None)` when option is not present.
    ///
    /// [`value_from_fn`]: struct.Arguments.html#method.value_from_fn
    pub fn opt_value_from_fn<A: Into<Keys>, T, E: Display>(
        &mut self,
        keys: A,
        f: fn(&str) -> Result<T, E>,
    ) -> Result<Option<T>, Error> {
        self.opt_value_from_fn_impl(keys.into(), f)
    }

    #[inline(never)]
    fn opt_value_from_fn_impl<T, E: Display>(
        &mut self,
        keys: Keys,
        f: fn(&str) -> Result<T, E>,
    ) -> Result<Option<T>, Error> {
        match self.find_value(keys)? {
            Some((value, kind, idx)) => {
                match f(value) {
                    Ok(value) => {
                        // Remove only when all checks are passed.
                        self.0.remove(idx);
                        if kind == PairKind::TwoArguments {
                            self.0.remove(idx);
                        }

                        Ok(Some(value))
                    }
                    Err(e) => Err(Error::Utf8ArgumentParsingFailed {
                        value: value.to_string(),
                        cause: error_to_string(e),
                    }),
                }
            }
            None => Ok(None),
        }
    }

    // The whole logic must be type-independent to prevent monomorphization.
    #[inline(never)]
    fn find_value(&mut self, keys: Keys) -> Result<Option<(&str, PairKind, usize)>, Error> {
        if let Some((idx, key)) = self.index_of(keys) {
            // Parse a `--key value` pair.

            let value = match self.0.get(idx + 1) {
                Some(v) => v,
                None => return Err(Error::OptionWithoutAValue(key)),
            };

            let value = os_to_str(value)?;
            Ok(Some((value, PairKind::TwoArguments, idx)))
        } else {
            Ok(None)
        }
    }

    /// Parses multiple key-value pairs into the `Vec` using `FromStr` trait.
    ///
    /// This is a shorthand for `values_from_fn("--key", FromStr::from_str)`
    pub fn values_from_str<A, T>(&mut self, keys: A) -> Result<Vec<T>, Error>
    where
        A: Into<Keys>,
        T: FromStr,
        <T as FromStr>::Err: Display,
    {
        self.values_from_fn(keys, FromStr::from_str)
    }

    /// Parses multiple key-value pairs into the `Vec` using a specified function.
    ///
    /// This functions can be used to parse arguments like:<br>
    /// `--file /path1 --file /path2 --file /path3`<br>
    /// But not `--file /path1 /path2 /path3`.
    ///
    /// Arguments can also be separated: `--file /path1 --some-flag --file /path2`
    ///
    /// This method simply executes [`opt_value_from_fn`] multiple times.
    ///
    /// An empty `Vec` is not an error.
    ///
    /// [`opt_value_from_fn`]: struct.Arguments.html#method.opt_value_from_fn
    pub fn values_from_fn<A: Into<Keys>, T, E: Display>(
        &mut self,
        keys: A,
        f: fn(&str) -> Result<T, E>,
    ) -> Result<Vec<T>, Error> {
        let keys = keys.into();

        let mut values = Vec::new();
        loop {
            match self.opt_value_from_fn(keys, f) {
                Ok(Some(v)) => values.push(v),
                Ok(None) => break,
                Err(e) => return Err(e),
            }
        }

        Ok(values)
    }

    /// Parses a key-value pair using a specified function.
    ///
    /// Unlike [`value_from_fn`], parses `&OsStr` and not `&str`.
    ///
    /// Must be used only once for each option.
    ///
    /// # Errors
    ///
    /// - When option is not present.
    /// - When value parsing failed.
    /// - When key-value pair is separated not by space.
    ///   Only [`value_from_fn`] supports `=` separator.
    ///
    /// [`value_from_fn`]: struct.Arguments.html#method.value_from_fn
    pub fn value_from_os_str<A: Into<Keys>, T, E: Display>(
        &mut self,
        keys: A,
        f: fn(&OsStr) -> Result<T, E>,
    ) -> Result<T, Error> {
        let keys = keys.into();
        match self.opt_value_from_os_str(keys, f) {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Err(Error::MissingOption(keys)),
            Err(e) => Err(e),
        }
    }

    /// Parses an optional key-value pair using a specified function.
    ///
    /// The same as [`value_from_os_str`], but returns `Ok(None)` when option is not present.
    ///
    /// [`value_from_os_str`]: struct.Arguments.html#method.value_from_os_str
    pub fn opt_value_from_os_str<A: Into<Keys>, T, E: Display>(
        &mut self,
        keys: A,
        f: fn(&OsStr) -> Result<T, E>,
    ) -> Result<Option<T>, Error> {
        self.opt_value_from_os_str_impl(keys.into(), f)
    }

    #[inline(never)]
    fn opt_value_from_os_str_impl<T, E: Display>(
        &mut self,
        keys: Keys,
        f: fn(&OsStr) -> Result<T, E>,
    ) -> Result<Option<T>, Error> {
        if let Some((idx, key)) = self.index_of(keys) {
            // Parse a `--key value` pair.

            let value = match self.0.get(idx + 1) {
                Some(v) => v,
                None => return Err(Error::OptionWithoutAValue(key)),
            };

            match f(value) {
                Ok(value) => {
                    // Remove only when all checks are passed.
                    self.0.remove(idx);
                    self.0.remove(idx);
                    Ok(Some(value))
                }
                Err(e) => Err(Error::ArgumentParsingFailed { cause: error_to_string(e) }),
            }
        } else {
            Ok(None)
        }
    }

    /// Parses multiple key-value pairs into the `Vec` using a specified function.
    ///
    /// This method simply executes [`opt_value_from_os_str`] multiple times.
    ///
    /// Unlike [`values_from_fn`], parses `&OsStr` and not `&str`.
    ///
    /// An empty `Vec` is not an error.
    ///
    /// [`opt_value_from_os_str`]: struct.Arguments.html#method.opt_value_from_os_str
    /// [`values_from_fn`]: struct.Arguments.html#method.values_from_fn
    pub fn values_from_os_str<A: Into<Keys>, T, E: Display>(
        &mut self,
        keys: A,
        f: fn(&OsStr) -> Result<T, E>,
    ) -> Result<Vec<T>, Error> {
        let keys = keys.into();
        let mut values = Vec::new();
        loop {
            match self.opt_value_from_os_str(keys, f) {
                Ok(Some(v)) => values.push(v),
                Ok(None) => break,
                Err(e) => return Err(e),
            }
        }

        Ok(values)
    }

    #[inline(never)]
    fn index_of(&self, keys: Keys) -> Option<(usize, &'static str)> {
        // Do not unroll loop to save space, because it creates a bigger file.
        // Which is strange, since `index_of2` actually benefits from it.

        for key in &keys.0 {
            if !key.is_empty()
                && let Some(i) = self.0.iter().position(|v| v == key)
            {
                return Some((i, key));
            }
        }

        None
    }

    /// Parses a free-standing argument using `FromStr` trait.
    ///
    /// This is a shorthand for `free_from_fn(FromStr::from_str)`
    pub fn free_from_str<T>(&mut self) -> Result<T, Error>
    where
        T: FromStr,
        <T as FromStr>::Err: Display,
    {
        self.free_from_fn(FromStr::from_str)
    }

    /// Parses a free-standing argument using a specified function.
    ///
    /// Parses the first argument from the list of remaining arguments.
    /// Therefore, it's up to the caller to check if the argument is actually
    /// a free-standing one and not an unused flag/option.
    ///
    /// Sadly, there is no way to automatically check for flag/option.
    /// `-`, `--`, `-1`, `-0.5`, `--.txt` - all of this arguments can have different
    /// meaning depending on the caller requirements.
    ///
    /// Must be used only once for each argument.
    ///
    /// # Errors
    ///
    /// - When argument is not a UTF-8 string. Use [`free_from_os_str`] instead.
    /// - When argument parsing failed.
    /// - When argument is not present.
    ///
    /// [`free_from_os_str`]: struct.Arguments.html#method.free_from_os_str
    #[inline(never)]
    pub fn free_from_fn<T, E: Display>(&mut self, f: fn(&str) -> Result<T, E>) -> Result<T, Error> {
        self.opt_free_from_fn(f)?.ok_or(Error::MissingArgument)
    }

    /// Parses a free-standing argument using a specified function.
    ///
    /// The same as [`free_from_fn`], but parses `&OsStr` instead of `&str`.
    ///
    /// [`free_from_fn`]: struct.Arguments.html#method.free_from_fn
    #[inline(never)]
    pub fn free_from_os_str<T, E: Display>(
        &mut self,
        f: fn(&OsStr) -> Result<T, E>,
    ) -> Result<T, Error> {
        self.opt_free_from_os_str(f)?.ok_or(Error::MissingArgument)
    }

    /// Parses an optional free-standing argument using `FromStr` trait.
    ///
    /// The same as [`free_from_str`], but returns `Ok(None)` when argument is not present.
    ///
    /// [`free_from_str`]: struct.Arguments.html#method.free_from_str
    pub fn opt_free_from_str<T>(&mut self) -> Result<Option<T>, Error>
    where
        T: FromStr,
        <T as FromStr>::Err: Display,
    {
        self.opt_free_from_fn(FromStr::from_str)
    }

    /// Parses an optional free-standing argument using a specified function.
    ///
    /// The same as [`free_from_fn`], but returns `Ok(None)` when argument is not present.
    ///
    /// [`free_from_fn`]: struct.Arguments.html#method.free_from_fn
    #[inline(never)]
    pub fn opt_free_from_fn<T, E: Display>(
        &mut self,
        f: fn(&str) -> Result<T, E>,
    ) -> Result<Option<T>, Error> {
        if self.0.is_empty() {
            Ok(None)
        } else {
            let value = self.0.remove(0);
            let value = os_to_str(value.as_os_str())?;
            match f(value) {
                Ok(value) => Ok(Some(value)),
                Err(e) => Err(Error::Utf8ArgumentParsingFailed {
                    value: value.to_string(),
                    cause: error_to_string(e),
                }),
            }
        }
    }

    /// Parses a free-standing argument using a specified function.
    ///
    /// The same as [`free_from_os_str`], but returns `Ok(None)` when argument is not present.
    ///
    /// [`free_from_os_str`]: struct.Arguments.html#method.free_from_os_str
    #[inline(never)]
    pub fn opt_free_from_os_str<T, E: Display>(
        &mut self,
        f: fn(&OsStr) -> Result<T, E>,
    ) -> Result<Option<T>, Error> {
        if self.0.is_empty() {
            Ok(None)
        } else {
            let value = self.0.remove(0);
            match f(value.as_os_str()) {
                Ok(value) => Ok(Some(value)),
                Err(e) => Err(Error::ArgumentParsingFailed { cause: error_to_string(e) }),
            }
        }
    }

    /// Returns a list of remaining arguments.
    ///
    /// It's up to the caller what to do with them.
    /// One can report an error about unused arguments,
    /// other can use them for further processing.
    pub fn finish(self) -> Vec<OsString> {
        self.0
    }
}

// Display::to_string() is usually inlined, so by wrapping it in a non-inlined
// function we are reducing the size a bit.
#[inline(never)]
fn error_to_string<E: Display>(e: E) -> String {
    e.to_string()
}

#[inline]
fn os_to_str(text: &OsStr) -> Result<&str, Error> {
    text.to_str().ok_or(Error::NonUtf8Argument)
}

/// A keys container.
///
/// Should not be used directly.
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct Keys([&'static str; 2]);

impl Keys {
    #[inline]
    fn first(&self) -> &'static str {
        self.0[0]
    }

    #[inline]
    fn second(&self) -> &'static str {
        self.0[1]
    }
}

impl From<[&'static str; 2]> for Keys {
    #[inline]
    fn from(v: [&'static str; 2]) -> Self {
        debug_assert!(v[0].starts_with("-"), "an argument should start with '-'");
        validate_shortflag(v[0]);
        debug_assert!(!v[0].starts_with("--"), "the first argument should be short");
        debug_assert!(v[1].starts_with("--"), "the second argument should be long");
        Keys(v)
    }
}

fn validate_shortflag(short_key: &'static str) {
    let mut chars = short_key[1..].chars();
    if let Some(first) = chars.next() {
        debug_assert!(
            short_key.len() == 2 || chars.all(|c| c == first),
            "short keys should be a single character or a repeated character"
        );
    }
}

impl From<&'static str> for Keys {
    #[inline]
    fn from(v: &'static str) -> Self {
        debug_assert!(v.starts_with("-"), "an argument should start with '-'");
        if !v.starts_with("--") {
            validate_shortflag(v);
        }
        Keys([v, ""])
    }
}
