extern crate backtrace;
#[macro_use]
extern crate lazy_static;

use std::{
    collections::HashSet,
    io::Write,
    os::{raw::c_void, unix::io::FromRawFd},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

lazy_static! {
    static ref STORAGESTUDY_RESOLVED_ADDRS: Mutex<HashSet<usize>> = Mutex::new(HashSet::new());
    static ref STORAGESTUDY_TRACE: Mutex<std::fs::File> =
        Mutex::new(unsafe { std::fs::File::from_raw_fd(3) });
}

pub fn era(era: &str) {
    writeln!(
        *STORAGESTUDY_TRACE.lock().unwrap(),
        "[storagestudy] era {}",
        era
    )
    .unwrap();
}

pub fn dump(op: &str) {
    let bt = backtrace::Backtrace::new_unresolved();
    let mut chain = vec![0; bt.frames().len()];
    for (i, frame) in bt.frames().iter().enumerate() {
        let ip = frame.ip() as usize;
        chain[i] = ip;
        let need_resolve = {
            let mut guard = STORAGESTUDY_RESOLVED_ADDRS.lock().unwrap();
            if guard.contains(&ip) {
                false
            } else {
                guard.insert(ip);
                true
            }
        };
        if need_resolve {
            writeln!(
                *STORAGESTUDY_TRACE.lock().unwrap(),
                "[storagestudy] new-ip {:?}",
                ip
            )
            .unwrap();
            backtrace::resolve(ip as *mut c_void, |symbol: &backtrace::Symbol| {
                writeln!(
                    *STORAGESTUDY_TRACE.lock().unwrap(),
                    "[storagestudy] ip-symbol {:?} {:?}",
                    ip,
                    symbol
                )
                .unwrap();
            });
        }
    }
    writeln!(
        *STORAGESTUDY_TRACE.lock().unwrap(),
        "[storagestudy] {} {:?}",
        op,
        &chain[..]
    )
    .unwrap();
}
