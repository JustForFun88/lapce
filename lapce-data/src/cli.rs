use crate::editor::LineCol;
use std::{path::PathBuf, path::Path};
use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub enum PathObject {
    File(PathBuf, Option<LineCol>),
    Directory(PathBuf),
}

impl clap::builder::ValueParserFactory for PathObject {
    type Parser = PathObjectParser;
    fn value_parser() -> Self::Parser {
        PathObjectParser
    }
}

#[derive(Clone, Debug)]
pub struct PathObjectParser;

impl PathObjectParser {
    #[inline]
    fn parse_path(path: &Path) -> Option<PathObject> {
        fn write_text_with_sep_to<'a, I>(mut iter: I, buf: &mut String, sep: &str)
        where
            I: Iterator<Item = &'a str>,
        {
            if let Some(str) = iter.next() {
                buf.push_str(str);
                buf.push_str(sep);
                write_text_with_sep_to(iter, buf, sep);
            }
        }

        if let Ok(path_buf) = path.canonicalize() {
            if path_buf.is_file() {
                return Some(PathObject::File(path_buf, None));
            } else if path_buf.is_dir() {
                return Some(PathObject::Directory(path_buf));
            }
        }

        if let Some(str) = path.to_str() {
            let mut splits = str.rsplit(':');
            if let Some(first_rhs) = splits.next() {
                if let Ok(first_rhs_num) = first_rhs.parse::<usize>() {
                    if let Some(second_rhs) = splits.next() {
                        match second_rhs.parse::<usize>() {
                            Ok(second_rhs_num) => {
                                let mut str = String::new();
                                write_text_with_sep_to(splits.rev(), &mut str, ":");

                                if let Ok(left_path) =
                                    PathBuf::from(&str[..str.len() - 1])
                                        .canonicalize()
                                {
                                    if left_path.is_file() {
                                        return Some(PathObject::File(
                                            left_path,
                                            Some(LineCol {
                                                line: second_rhs_num,
                                                column: first_rhs_num,
                                            }),
                                        ));
                                    }
                                }
                                // We have some second right hand number, but the remaining path isn't a file,
                                // then let's check if it changed if we add `second_rhs`?
                                // Last char of `str` is ":", so we neen to push only `second_rhs`
                                str.push_str(second_rhs);
                                if let Ok(left_path) =
                                    PathBuf::from(str).canonicalize()
                                {
                                    if left_path.is_file() {
                                        return Some(PathObject::File(
                                            left_path,
                                            Some(LineCol {
                                                line: first_rhs_num,
                                                column: 1,
                                            }),
                                        ));
                                    }
                                }
                            }
                            Err(_) => {
                                let mut str = String::new();
                                write_text_with_sep_to(splits.rev(), &mut str, ":");
                                str.push_str(second_rhs);

                                if let Ok(left_path) =
                                    PathBuf::from(str).canonicalize()
                                {
                                    if left_path.is_file() {
                                        return Some(PathObject::File(
                                            left_path,
                                            Some(LineCol {
                                                line: first_rhs_num,
                                                column: 1,
                                            }),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Ignore the path if it doesn't refer to a file
        None
    }
}

impl clap::builder::TypedValueParser for PathObjectParser {
    type Value = PathObject;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        use clap::error::{Error, ErrorKind};
        if value.is_empty() {
            return Err({
                let arg = arg
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "...".to_owned());
                Error::raw(ErrorKind::EmptyValue, format!("InvalidArg {}", arg))
            });
        }

        let path = Path::new(value);
        // If path is absolute just call parse_path without any changes of path.
        // Else add path to the `std::env::current_dir()` and dp the same. None that
        // in both cases `path` can be as file as well as directory.
        if path.is_absolute() {
            if let Some(path_buf) = Self::parse_path(path) {
                return Ok(path_buf);
            }
        } else {
            static BASE: Lazy<PathBuf> =
                Lazy::new(|| std::env::current_dir().unwrap_or_default());
            let path = BASE.join(path);
            if let Some(path_buf) = Self::parse_path(&path) {
                return Ok(path_buf);
            }
        }
        // Return an error if we couldn't parse the path
        Err({
            let arg = arg
                .map(ToString::to_string)
                .unwrap_or_else(|| "...".to_owned());
            Error::raw(ErrorKind::InvalidValue, format!("InvalidArg {}", arg))
        })
    }
}
