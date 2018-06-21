/// temporary fix: define macros that match the log crate interface but simply use println directly
/// once log is enabled for enclaves, we can simply swap in the real macros

// max log level defined at compile-time
#[cfg(feature = "benchmark")]
pub const STATIC_MAX_LEVEL: ::log::LevelFilter = ::log::LevelFilter::Error;

#[cfg(not(feature = "benchmark"))]
pub const STATIC_MAX_LEVEL: ::log::LevelFilter = ::log::LevelFilter::Trace;

macro_rules! log {
    (target: $target:expr, $lvl:expr, $($arg:tt)+) => ({
        let lvl = $lvl;
        if lvl <= $crate::logger::STATIC_MAX_LEVEL {
            println!($($arg)+)
        }
    });
    ($lvl:expr, $($arg:tt)+) => (log!(target: module_path!(), $lvl, $($arg)+))
}

macro_rules! error {
    (target: $target:expr, $($arg:tt)*) => (
        log!(target: $target, $crate::log::Level::Error, $($arg)*);
    );
    ($($arg:tt)*) => (
        log!($crate::log::Level::Error, $($arg)*);
    )
}

macro_rules! warn {
    (target: $target:expr, $($arg:tt)*) => (
        log!(target: $target, $crate::log::Level::Warn, $($arg)*);
    );
    ($($arg:tt)*) => (
        log!($crate::log::Level::Warn, $($arg)*);
    )
}

macro_rules! info {
    (target: $target:expr, $($arg:tt)*) => (
        log!(target: $target, $crate::log::Level::Info, $($arg)*);
    );
    ($($arg:tt)*) => (
        log!($crate::log::Level::Info, $($arg)*);
    )
}

macro_rules! debug {
    (target: $target:expr, $($arg:tt)*) => (
        log!(target: $target, $crate::log::Level::Debug, $($arg)*);
    );
    ($($arg:tt)*) => (
        log!($crate::log::Level::Debug, $($arg)*);
    )
}

macro_rules! trace {
    (target: $target:expr, $($arg:tt)*) => (
        log!(target: $target, $crate::log::Level::Trace, $($arg)*);
    );
    ($($arg:tt)*) => (
        log!($crate::log::Level::Trace, $($arg)*);
    )
}
