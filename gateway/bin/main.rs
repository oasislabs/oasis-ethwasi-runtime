// Copyright 2015-2018 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Ethcore client application.

#![warn(missing_docs)]

extern crate ctrlc;
extern crate fdlimit;
#[macro_use]
extern crate log;
extern crate parking_lot;

extern crate web3_gateway;

use ctrlc::CtrlC;
use fdlimit::raise_fd_limit;
use parking_lot::{Condvar, Mutex};
use std::sync::Arc;

use web3_gateway::start;

// Run our version of parity.
fn main() {
    // increase max number of open files
    raise_fd_limit();

    let exit = Arc::new((Mutex::new(false), Condvar::new()));

    let client = start().unwrap();

    CtrlC::set_handler({
        let e = exit.clone();
        move || {
            e.1.notify_all();
        }
    });

    // Wait for signal
    let mut lock = exit.0.lock();
    let _ = exit.1.wait(&mut lock);

    client.shutdown();
}
