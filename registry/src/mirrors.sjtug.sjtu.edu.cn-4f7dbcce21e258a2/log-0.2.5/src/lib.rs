// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A lightweight logging facade.
//!
//! A logging facade provides a single logging API that abstracts over the
//! actual logging implementation. Libraries can use the logging API provided
//! by this crate, and the consumer of those libraries can choose the logging
//! framework that is most suitable for its use case.
//!
//! If no logging implementation is selected, the facade falls back to a "noop"
//! implementation that ignores all log messages. The overhead in this case
//! is very small - just an integer load, comparison and jump.
//!
//! # Use
//!
//! ## In libraries
//!
//! Libraries should link only to the `log` crate, and use the provided
//! macros to log whatever information will be useful to downstream consumers.
//!
//! ### Examples
//!
//! ```rust
//! # #![allow(unstable)]
//! #[macro_use]
//! extern crate log;
//!
//! # pub struct Yak(String);
//! # impl Yak { fn shave(&self, _: u32) {} }
//! # fn find_a_razor() -> Result<u32, u32> { Ok(1) }
//! pub fn shave_the_yak(yak: &Yak) {
//!     trace!("Commencing yak shaving");
//!
//!     loop {
//!         match find_a_razor() {
//!             Ok(razor) => {
//!                 info!("Razor located: {}", razor);
//!                 yak.shave(razor);
//!                 break;
//!             }
//!             Err(err) => {
//!                 warn!("Unable to locate a razor: {}, retrying", err);
//!             }
//!         }
//!     }
//! }
//! # fn main() {}
//! ```
//!
//! ## In executables
//!
//! Executables should chose a logging framework and initialize it early in the
//! runtime of the program. Logging frameworks will typically include a
//! function to do this. Any log messages generated before the framework is
//! initialized will be ignored.
//!
//! The executable itself may use the `log` crate to log as well.
//!
//! ### Warning
//!
//! The logging system may only be initialized once.
//!
//! ### Examples
//!
//! ```rust,ignore
//! #[macro_use]
//! extern crate log;
//! extern crate my_logger;
//!
//! fn main() {
//!     my_logger::init();
//!
//!     info!("starting up");
//!
//!     // ...
//! }
//! ```
//!
//! # Logger implementations
//!
//! Loggers implement the `Log` trait. Here's a very basic example that simply
//! logs all messages at the `Error`, `Warn` or `Info` levels to stdout:
//!
//! ```rust
//! extern crate log;
//!
//! use log::{LogRecord, LogLevel};
//!
//! struct SimpleLogger;
//!
//! impl log::Log for SimpleLogger {
//!     fn enabled(&self, level: LogLevel, _module: &str) -> bool {
//!         level <= LogLevel::Info
//!     }
//!
//!     fn log(&self, record: &LogRecord) {
//!         if self.enabled(record.level(), record.location().module_path) {
//!             println!("{} - {}", record.level(), record.args());
//!         }
//!     }
//! }
//!
//! # fn main() {}
//! ```
//!
//! Loggers are installed by calling the `set_logger` function. It takes a
//! closure which is provided a `MaxLogLevel` token and returns a `Log` trait
//! object. The `MaxLogLevel` token controls the global maximum log level. The
//! logging facade uses this as an optimization to improve performance of log
//! messages at levels that are disabled. In the case of our example logger,
//! we'll want to set the maximum log level to `Info`, since we ignore any
//! `Debug` or `Trace` level log messages. A logging framework should provide a
//! function that wraps a call to `set_logger`, handling initialization of the
//! logger:
//!
//! ```rust
//! # extern crate log;
//! # use log::{LogLevel, LogLevelFilter, SetLoggerError};
//! # struct SimpleLogger;
//! # impl log::Log for SimpleLogger {
//! #   fn enabled(&self, _: LogLevel, _: &str) -> bool { false }
//! #   fn log(&self, _: &log::LogRecord) {}
//! # }
//! # fn main() {}
//! pub fn init() -> Result<(), SetLoggerError> {
//!     log::set_logger(|max_log_level| {
//!         max_log_level.set(LogLevelFilter::Info);
//!         Box::new(SimpleLogger)
//!     })
//! }
//! ```

#![doc(html_logo_url = "http://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
       html_favicon_url = "http://www.rust-lang.org/favicon.ico",
       html_root_url = "http://doc.rust-lang.org/log/")]
#![warn(missing_docs)]
#![feature(core, std_misc)]

use std::ascii::AsciiExt;
use std::cmp;
use std::error;
use std::fmt;
use std::mem;
use std::ops::Deref;
use std::rt;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
mod macros;

// The setup here is a bit weird to make at_exit work.
//
// There are four different states that we care about: the logger's
// uninitialized, the logger's initializing (set_logger's been called but
// LOGGER hasn't actually been set yet), the logger's active, or the logger's
// shutting down inside of at_exit.
//
// The LOGGER static is normally a Box<Box<Log>> with some special possible
// values as well. The uninitialized and initializing states are represented by
// the values 0 and 1 respectively. The shutting down state is also represented
// by 1. Any other value is a valid pointer to the logger.
//
// The at_exit routine needs to make sure that no threads are actively logging
// when it deallocates the logger. The number of actively logging threads is
// tracked in the REFCOUNT static. The routine first sets LOGGER back to 1.
// All logging calls past that point will immediately return without accessing
// the logger. At that point, the at_exit routine just waits for the refcount
// to reach 0 before deallocating the logger. Note that the refcount does not
// necessarily monotonically decrease at this point, as new log calls still
// increment and decrement it, but the interval in between is small enough that
// the wait is really just for the active log calls to finish.
static LOGGER: AtomicUsize = ATOMIC_USIZE_INIT;
static REFCOUNT: AtomicUsize = ATOMIC_USIZE_INIT;

const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;

static MAX_LOG_LEVEL_FILTER: AtomicUsize = ATOMIC_USIZE_INIT;

static LOG_LEVEL_NAMES: [&'static str; 6] = ["OFF", "ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

/// An enum representing the available verbosity levels of the logging framework
///
/// A `LogLevel` may be compared directly to a `LogLevelFilter`.
#[repr(usize)]
#[derive(Copy, Eq, Debug)]
pub enum LogLevel {
    /// The "error" level.
    ///
    /// Designates very serious errors.
    Error = 1, // This way these line up with the discriminants for LogLevelFilter below
    /// The "warn" level.
    ///
    /// Designates hazardous situations.
    Warn,
    /// The "info" level.
    ///
    /// Designates useful information.
    Info,
    /// The "debug" level.
    ///
    /// Designates lower priority information.
    Debug,
    /// The "trace" level.
    ///
    /// Designates very low priority, often extremely verbose, information.
    Trace,
}

impl Clone for LogLevel {
    #[inline]
    fn clone(&self) -> LogLevel {
        *self
    }
}

impl PartialEq for LogLevel {
    #[inline]
    fn eq(&self, other: &LogLevel) -> bool {
        *self as usize == *other as usize
    }
}

impl PartialEq<LogLevelFilter> for LogLevel {
    #[inline]
    fn eq(&self, other: &LogLevelFilter) -> bool {
        *self as usize == *other as usize
    }
}

impl PartialOrd for LogLevel {
    #[inline]
    fn partial_cmp(&self, other: &LogLevel) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd<LogLevelFilter> for LogLevel {
    #[inline]
    fn partial_cmp(&self, other: &LogLevelFilter) -> Option<cmp::Ordering> {
        Some((*self as usize).cmp(&(*other as usize)))
    }
}

impl Ord for LogLevel {
    #[inline]
    fn cmp(&self, other: &LogLevel) -> cmp::Ordering {
        (*self as usize).cmp(&(*other as usize))
    }
}

impl FromStr for LogLevel {
    type Err = ();
    fn from_str(level: &str) -> Result<LogLevel, ()> {
        LOG_LEVEL_NAMES.iter()
            .position(|&name| name.eq_ignore_ascii_case(level))
            .into_iter()
            .filter(|&idx| idx != 0)
            .map(|idx| LogLevel::from_usize(idx).unwrap())
            .next()
            .ok_or(())
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", LOG_LEVEL_NAMES[*self as usize])
    }
}

impl LogLevel {
    fn from_usize(u: usize) -> Option<LogLevel> {
        match u {
            1 => Some(LogLevel::Error),
            2 => Some(LogLevel::Warn),
            3 => Some(LogLevel::Info),
            4 => Some(LogLevel::Debug),
            5 => Some(LogLevel::Trace),
            _ => None
        }
    }

    /// Returns the most verbose logging level.
    #[inline]
    pub fn max() -> LogLevel {
        LogLevel::Trace
    }

    /// Converts the `LogLevel` to the equivalent `LogLevelFilter`.
    #[inline]
    pub fn to_log_level_filter(&self) -> LogLevelFilter {
        LogLevelFilter::from_usize(*self as usize).unwrap()
    }
}

/// An enum representing the available verbosity level filters of the logging
/// framework.
///
/// A `LogLevelFilter` may be compared directly to a `LogLevel`.
#[repr(usize)]
#[derive(Copy, Eq, Debug)]
pub enum LogLevelFilter {
    /// A level lower than all log levels.
    Off,
    /// Corresponds to the `Error` log level.
    Error,
    /// Corresponds to the `Warn` log level.
    Warn,
    /// Corresponds to the `Trace` log level.
    Info,
    /// Corresponds to the `Debug` log level.
    Debug,
    /// Corresponds to the `Trace` log level.
    Trace,
}

// Deriving generates terrible impls of these traits

impl Clone for LogLevelFilter {
    #[inline]
    fn clone(&self) -> LogLevelFilter {
        *self
    }
}

impl PartialEq for LogLevelFilter {
    #[inline]
    fn eq(&self, other: &LogLevelFilter) -> bool {
        *self as usize == *other as usize
    }
}

impl PartialEq<LogLevel> for LogLevelFilter {
    #[inline]
    fn eq(&self, other: &LogLevel) -> bool {
        other.eq(self)
    }
}

impl PartialOrd for LogLevelFilter {
    #[inline]
    fn partial_cmp(&self, other: &LogLevelFilter) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd<LogLevel> for LogLevelFilter {
    #[inline]
    fn partial_cmp(&self, other: &LogLevel) -> Option<cmp::Ordering> {
        other.partial_cmp(self).map(|x| x.reverse())
    }
}

impl Ord for LogLevelFilter {
    #[inline]
    fn cmp(&self, other: &LogLevelFilter) -> cmp::Ordering {
        (*self as usize).cmp(&(*other as usize))
    }
}

impl FromStr for LogLevelFilter {
    type Err = ();
    fn from_str(level: &str) -> Result<LogLevelFilter, ()> {
        LOG_LEVEL_NAMES.iter()
            .position(|&name| name.eq_ignore_ascii_case(level))
            .map(|p| LogLevelFilter::from_usize(p).unwrap())
            .ok_or(())
    }
}

impl fmt::Display for LogLevelFilter {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", LOG_LEVEL_NAMES[*self as usize])
    }
}

impl LogLevelFilter {
    fn from_usize(u: usize) -> Option<LogLevelFilter> {
        match u {
            0 => Some(LogLevelFilter::Off),
            1 => Some(LogLevelFilter::Error),
            2 => Some(LogLevelFilter::Warn),
            3 => Some(LogLevelFilter::Info),
            4 => Some(LogLevelFilter::Debug),
            5 => Some(LogLevelFilter::Trace),
            _ => None
        }
    }
    /// Returns the most verbose logging level filter.
    #[inline]
    pub fn max() -> LogLevelFilter {
        LogLevelFilter::Trace
    }

    /// Converts `self` to the equivalent `LogLevel`.
    ///
    /// Returns `None` if `self` is `LogLevel::Off`.
    #[inline]
    pub fn to_log_level(&self) -> Option<LogLevel> {
        LogLevel::from_usize(*self as usize)
    }
}

/// The "payload" of a log message.
pub struct LogRecord<'a> {
    level: LogLevel,
    location: &'a LogLocation,
    args: fmt::Arguments<'a>,
}

impl<'a> LogRecord<'a> {
    /// Creates a new `LogRecord`.
    pub fn new(level: LogLevel, location: &'a LogLocation, args: fmt::Arguments<'a>)
               -> LogRecord<'a> {
        LogRecord {
            level: level,
            location: location,
            args: args,
        }
    }

    /// The message body.
    pub fn args(&self) -> &fmt::Arguments<'a> {
        &self.args
    }

    /// The location of the log directive.
    pub fn location(&self) -> &LogLocation {
        self.location
    }

    /// The verbosity level of the message.
    pub fn level(&self) -> LogLevel {
        self.level
    }
}

/// A trait encapsulating the operations required of a logger
pub trait Log: Sync+Send {
    /// Determines if a log message sent at the specified level from the
    /// specified module would be logged.
    ///
    /// This is used by the `log_enabled!` macro to allow callers to avoid
    /// expensive computation of log message arguments if the message would be
    /// discarded anyway.
    fn enabled(&self, level: LogLevel, module: &str) -> bool;

    /// Logs the `LogRecord`.
    ///
    /// Note that `enabled` is *not* necessarily called before this method.
    /// Implementations of `log` should perform all necessary filtering
    /// internally.
    fn log(&self, record: &LogRecord);
}

/// The location of a log message.
#[derive(Copy, Clone, Debug)]
pub struct LogLocation {
    /// The module path of the message.
    pub module_path: &'static str,
    /// The source file containing the message.
    pub file: &'static str,
    /// The line containing the message.
    pub line: u32,
}

/// A token providing read and write access to the global maximum log level
/// filter.
///
/// The maximum log level is used as an optimization to avoid evaluating log
/// messages that will be ignored by the logger. Any message with a level
/// higher than the maximum log level filter will be ignored. A logger should
/// make sure to keep the maximum log level filter in sync with its current
/// configuration.
#[allow(missing_copy_implementations)]
pub struct MaxLogLevelFilter(());

impl fmt::Debug for MaxLogLevelFilter {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "MaxLogLevelFilter")
    }
}

impl MaxLogLevelFilter {
    /// Gets the current maximum log level filter.
    pub fn get(&self) -> LogLevelFilter {
        max_log_level()
    }

    /// Sets the maximum log level.
    pub fn set(&self, level: LogLevelFilter) {
        MAX_LOG_LEVEL_FILTER.store(level as usize, Ordering::SeqCst)
    }
}

/// Returns the current maximum log level.
///
/// The `log!`, `error!`, `warn!`, `info!`, `debug!`, and `trace!` macros check
/// this value and discard any message logged at a higher level. The maximum
/// log level is set by the `MaxLogLevel` token passed to loggers.
#[inline(always)]
pub fn max_log_level() -> LogLevelFilter {
    unsafe { mem::transmute(MAX_LOG_LEVEL_FILTER.load(Ordering::Relaxed)) }
}

/// Sets the global logger.
///
/// The `make_logger` closure is passed a `MaxLogLevel` object, which the
/// logger should use to keep the global maximum log level in sync with the
/// highest log level that the logger will not ignore.
///
/// This function may only be called once in the lifetime of a program. Any log
/// events that occur before the call to `set_logger` completes will be
/// ignored.
///
/// This function does not typically need to be called manually. Logger
/// implementations should provide an initialization method that calls
/// `set_logger` internally.
pub fn set_logger<M>(make_logger: M) -> Result<(), SetLoggerError>
        where M: FnOnce(MaxLogLevelFilter) -> Box<Log> {
    if LOGGER.compare_and_swap(UNINITIALIZED, INITIALIZING, Ordering::SeqCst) != UNINITIALIZED {
        return Err(SetLoggerError(()));
    }

    let logger = Box::new(make_logger(MaxLogLevelFilter(())));
    let logger = unsafe { mem::transmute::<Box<Box<Log>>, usize>(logger) };
    LOGGER.store(logger, Ordering::SeqCst);
    rt::at_exit(|| {
        // Set to INITIALIZING to prevent re-initialization after
        let logger = LOGGER.swap(INITIALIZING, Ordering::SeqCst);

        while REFCOUNT.load(Ordering::SeqCst) != 0 {
            // FIXME add a sleep here when it doesn't involve timers
        }

        unsafe { mem::transmute::<usize, Box<Box<Log>>>(logger); }
    });

    Ok(())
}

/// The type returned by `set_logger` if `set_logger` has already been called.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct SetLoggerError(());

impl fmt::Display for SetLoggerError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "attempted to set a logger after the logging system was already initialized")
    }
}

impl error::Error for SetLoggerError {
    fn description(&self) -> &str { "set_logger() called multiple times" }
}

struct LoggerGuard(usize);

impl Drop for LoggerGuard {
    fn drop(&mut self) {
        REFCOUNT.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Deref for LoggerGuard {
    type Target = Box<Log>;

    fn deref(&self) -> &Box<Log+'static> {
        unsafe { mem::transmute(self.0) }
    }
}

fn logger() -> Option<LoggerGuard> {
    REFCOUNT.fetch_add(1, Ordering::SeqCst);
    let logger = LOGGER.load(Ordering::SeqCst);
    if logger == UNINITIALIZED || logger == INITIALIZING {
        REFCOUNT.fetch_sub(1, Ordering::SeqCst);
        None
    } else {
        Some(LoggerGuard(logger))
    }
}

/// Determines if the current logger will ignore a log message at the specified
/// level from the specified module.
///
/// This should not typically be called directly. The `log_enabled!` macro
/// should be used instead.
pub fn enabled(level: LogLevel, module: &str) -> bool {
    if let Some(logger) = logger() {
        logger.enabled(level, module)
    } else {
        false
    }
}

/// Logs a message.
///
/// This should not typically be called directly. The `log!`, `error!`,
/// `warn!`, `info!`, `debug!`, and `trace!` macros should be used instead.
pub fn log(level: LogLevel, loc: &LogLocation, args: fmt::Arguments) {
    if let Some(logger) = logger() {
        logger.log(&LogRecord::new(level, loc, args))
    }
}

#[cfg(test)]
mod tests {
     use std::error::Error;
     use super::{LogLevel, LogLevelFilter, SetLoggerError};

     #[test]
     fn test_loglevelfilter_from_str() {
         let tests = [
             ("off",   Ok(LogLevelFilter::Off)),
             ("error", Ok(LogLevelFilter::Error)),
             ("warn",  Ok(LogLevelFilter::Warn)),
             ("info",  Ok(LogLevelFilter::Info)),
             ("debug", Ok(LogLevelFilter::Debug)),
             ("trace", Ok(LogLevelFilter::Trace)),
             ("OFF",   Ok(LogLevelFilter::Off)),
             ("ERROR", Ok(LogLevelFilter::Error)),
             ("WARN",  Ok(LogLevelFilter::Warn)),
             ("INFO",  Ok(LogLevelFilter::Info)),
             ("DEBUG", Ok(LogLevelFilter::Debug)),
             ("TRACE", Ok(LogLevelFilter::Trace)),
             ("asdf",  Err(())),
         ];
         for &(s, ref expected) in tests.iter() {
             assert_eq!(expected, &s.parse());
         }
     }

     #[test]
     fn test_loglevel_from_str() {
         let tests = [
             ("OFF",   Err(())),
             ("error", Ok(LogLevel::Error)),
             ("warn",  Ok(LogLevel::Warn)),
             ("info",  Ok(LogLevel::Info)),
             ("debug", Ok(LogLevel::Debug)),
             ("trace", Ok(LogLevel::Trace)),
             ("ERROR", Ok(LogLevel::Error)),
             ("WARN",  Ok(LogLevel::Warn)),
             ("INFO",  Ok(LogLevel::Info)),
             ("DEBUG", Ok(LogLevel::Debug)),
             ("TRACE", Ok(LogLevel::Trace)),
             ("asdf",  Err(())),
         ];
         for &(s, ref expected) in tests.iter() {
             assert_eq!(expected, &s.parse());
         }
     }

     #[test]
     fn test_loglevel_show() {
         assert_eq!("INFO", LogLevel::Info.to_string());
         assert_eq!("ERROR", LogLevel::Error.to_string());
     }

     #[test]
     fn test_loglevelfilter_show() {
         assert_eq!("OFF", LogLevelFilter::Off.to_string());
         assert_eq!("ERROR", LogLevelFilter::Error.to_string());
     }

     #[test]
     fn test_cross_cmp() {
         assert!(LogLevel::Debug > LogLevelFilter::Error);
         assert!(LogLevelFilter::Warn < LogLevel::Trace);
         assert!(LogLevelFilter::Off < LogLevel::Error);
     }

     #[test]
     fn test_cross_eq() {
         assert!(LogLevel::Error == LogLevelFilter::Error);
         assert!(LogLevelFilter::Off != LogLevel::Error);
         assert!(LogLevel::Trace == LogLevelFilter::Trace);
     }

     #[test]
     fn test_to_log_level() {
         assert_eq!(Some(LogLevel::Error), LogLevelFilter::Error.to_log_level());
         assert_eq!(None, LogLevelFilter::Off.to_log_level());
         assert_eq!(Some(LogLevel::Debug), LogLevelFilter::Debug.to_log_level());
     }

     #[test]
     fn test_to_log_level_filter() {
         assert_eq!(LogLevelFilter::Error, LogLevel::Error.to_log_level_filter());
         assert_eq!(LogLevelFilter::Trace, LogLevel::Trace.to_log_level_filter());
     }

     #[test]
     fn test_error_trait() {
         let e = SetLoggerError(());
         assert_eq!(e.description(), "set_logger() called multiple times");
     }
}
