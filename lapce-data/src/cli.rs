use crate::editor::LineCol;
use clap::error::{Error, ErrorKind};
use core::num::ParseIntError;
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};

type Result<T> = core::result::Result<T, clap::Error>;

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

enum ParserError<'a> {
    InvalidPath,
    InvalidLine(&'a str, ParseIntError),
    InvalidColumn(&'a str, ParseIntError),
    InvalidLineColumn((&'a str, ParseIntError), (&'a str, ParseIntError)),
    NotFile(&'a str),
    NotFileOrDirectory,
    Other(&'a str, std::io::Error),
}

impl PathObjectParser {
    #[inline]
    fn parse_path(path: &Path) -> Result<PathObject> {
        static REG_LINE_COLUMN: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(.+):(\d+):(\d+)\z").unwrap());
        static REG_LINE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(.+):(\d+)\z").unwrap());

        // We shorten the parsing and see if the passed path is a valid file or directory.
        // If we don't succeed, then we move on. At this point, we will also catch all
        // possible file or directory names like "name:{line}:{column}", so in the future
        // this unlikely, but still possible option, can not be checked again.
        if let Ok(path_buf) = path.canonicalize() {
            if path_buf.is_file() {
                return Ok(PathObject::File(path_buf, None));
            } else if path_buf.is_dir() {
                return Ok(PathObject::Directory(path_buf));
            }
        }

        let path_str = match path.to_str() {
            Some(str) => str,
            None => return Err(Self::error(ParserError::InvalidPath, "")),
        };

        // Parsing if the passed path is "name:{line}:{column}"
        if let Some(captures) = REG_LINE_COLUMN.captures(path_str) {
            if let (Some(path), Some(line), Some(column)) =
                (captures.get(1), captures.get(2), captures.get(3))
            {
                let line_num = line.as_str().parse::<usize>();
                let column_num = column.as_str().parse::<usize>();
                match (line_num, column_num) {
                    (Ok(line_num), Ok(column_num)) => {
                        match PathBuf::from(path.as_str()).canonicalize() {
                            Ok(left_path) => {
                                return if left_path.is_file() {
                                    Ok(PathObject::File(
                                        left_path,
                                        Some(LineCol {
                                            line: line_num,
                                            column: column_num,
                                        }),
                                    ))
                                } else {
                                    Err(Self::error(
                                        ParserError::NotFile(path.as_str()),
                                        path_str,
                                    ))
                                }
                            }
                            Err(err) => {
                                // Here we're checking for an unlikely but still possible file
                                // name such as "name:number:{line}", where "name:number" is
                                // actually the filename without the extension.
                                let may_be_path = &path_str[..column.start() - 1];
                                if let Ok(left_path) =
                                    PathBuf::from(may_be_path).canonicalize()
                                {
                                    if left_path.is_file() {
                                        return Ok(PathObject::File(
                                            left_path,
                                            Some(LineCol {
                                                line: column_num,
                                                column: 1,
                                            }),
                                        ));
                                    }
                                }
                                // Path canonicalization failed for some reason, so let's just let the user know.
                                return Err(Self::error(
                                    ParserError::Other(path.as_str(), err),
                                    path_str,
                                ));
                            }
                        }
                    }
                    (Ok(_), Err(err)) => {
                        return Err(Self::error(
                            ParserError::InvalidColumn(column.as_str(), err),
                            path_str,
                        ))
                    }
                    (Err(err), Ok(_)) => {
                        return Err(Self::error(
                            ParserError::InvalidLine(line.as_str(), err),
                            path_str,
                        ));
                    }
                    (Err(line_err), Err(column_err)) => {
                        return Err(Self::error(
                            ParserError::InvalidLineColumn(
                                (line.as_str(), line_err),
                                (column.as_str(), column_err),
                            ),
                            path_str,
                        ));
                    }
                }
            }
        }

        // Parsing if the passed path is "name:{line}"
        if let Some(captures) = REG_LINE.captures(path_str) {
            if let (Some(path), Some(line)) = (captures.get(1), captures.get(2)) {
                match line.as_str().parse::<usize>() {
                    Ok(line_num) => {
                        if let Ok(left_path) =
                            PathBuf::from(path.as_str()).canonicalize()
                        {
                            return if left_path.is_file() {
                                Ok(PathObject::File(
                                    left_path,
                                    Some(LineCol {
                                        line: line_num,
                                        column: 1,
                                    }),
                                ))
                            } else {
                                Err(Self::error(
                                    ParserError::NotFile(path.as_str()),
                                    path_str,
                                ))
                            };
                        }
                    }
                    Err(err) => {
                        return Err(Self::error(
                            ParserError::InvalidLine(line.as_str(), err),
                            path_str,
                        ));
                    }
                }
            }
        }

        Err(Self::error(ParserError::NotFileOrDirectory, path_str))
    }

    fn error(error: ParserError, path_str: &str) -> clap::Error {
        match error {
            ParserError::InvalidPath => {
                Error::raw(ErrorKind::InvalidValue, "Invalid path")
            }
            ParserError::InvalidLine(line, parse_int_error) => {
                let message = format!(
                    "Invalid line in \"{}\", cannot parse \
                    \"{}\" as line number because of \"{}\"",
                    path_str, line, parse_int_error
                );
                Error::raw(ErrorKind::InvalidValue, message)
            }
            ParserError::InvalidColumn(column, parse_int_error) => {
                let message = format!(
                    "Invalid column in \"{}\", cannot parse \
                    \"{}\" as column number because of \"{}\"",
                    path_str, column, parse_int_error
                );
                Error::raw(ErrorKind::InvalidValue, message)
            }
            ParserError::InvalidLineColumn(
                (line, line_err),
                (column, column_err),
            ) => {
                let message = format!(
                    "Invalid line and column in \"{}\", cannot parse \
                    \"{}\" as line number because of \"{}\", cannot parse \
                    \"{}\" as column number because of \"{}\"",
                    path_str, line, line_err, column, column_err
                );
                Error::raw(ErrorKind::InvalidValue, message)
            }
            ParserError::NotFile(file_name) => {
                let message = format!(
                    "\"{}\" in the input arguments \"{}\" is not a file",
                    file_name, path_str
                );
                Error::raw(ErrorKind::InvalidValue, message)
            }
            ParserError::Other(path, err) => {
                let message = format!(
                    "Invalid path \"{}\" in the in the input arguments \"{}\", because of \"{}\"",
                    path, path_str, err
                );
                Error::raw(ErrorKind::InvalidValue, message)
            }
            ParserError::NotFileOrDirectory => {
                let message = format!("\"{}\" is not a file or directory", path_str);
                Error::raw(ErrorKind::InvalidValue, message)
            }
        }
    }
}

impl clap::builder::TypedValueParser for PathObjectParser {
    type Value = PathObject;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value> {
        if value.is_empty() {
            return Err({
                let arg = arg
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "...".to_owned());
                Error::raw(ErrorKind::EmptyValue, format!("InvalidArg: \"{}\"", arg))
            });
        }

        let path = Path::new(value);
        // If path is absolute just call parse_path without any changes of path.
        // Else add path to the `std::env::current_dir()` and do the same. None that
        // in both cases `path` can be as file as well as directory.
        if path.is_absolute() {
            Self::parse_path(path)
        } else {
            let BASE = std::env::current_dir().unwrap_or_default();
            // static BASE: Lazy<PathBuf> =
            //     Lazy::new(|| std::env::current_dir().unwrap_or_default());
            let path = BASE.join(path);
            Self::parse_path(&path)
        }
    }
}
