#![feature(try_from)]

extern crate ndarray;
#[macro_use]
extern crate tvm;

extern crate pwasm_ethereum;
extern crate pwasm_std;

use std::convert::TryInto;

use ndarray::Array;
use pwasm_std::logger::debug;
use tvm::{ffi::runtime::DLTensor,
          runtime::{Module, SystemLibModule}};

// This annotation to link the function with the correct external library is required.
// The name of the link is important as it must follow the name of the archive.
// The compile will look for lib<link_name>.a for the exported function to link.
#[link(name = "test", kind = "static")]
extern "C" {
    // This is an external function that is exported by TVM.
    // It registers the exported TVM functions specified in the model
    // and allow them to be fetched and called from within the contract.
    fn __tvm_module_startup();
}

#[no_mangle]
pub fn deploy() {}

#[no_mangle]
pub fn call() {
    // Must be run first to register the functions.
    unsafe {
        __tvm_module_startup();
    }

    let syslib = SystemLibModule::default();
    let add_one = syslib.get_function("add_one").unwrap();
    let mut a = Array::from_vec(vec![1f32, 0., 1., 2.]);
    let mut b = Array::from_vec(vec![0f32; 4]);
    let mut a_dl: DLTensor = (&mut a).into();
    let mut b_dl: DLTensor = (&mut b).into();

    let c = Array::from_vec(vec![2f32, 1., 2., 3.]);

    let _result: i32 = call_packed!(add_one, &mut a_dl, &mut b_dl)
        .try_into()
        .unwrap();
    // `debug` can be used display messages in the terminal
    debug(&format!("output: {:?}", b));
    assert!(c.all_close(&b, 1e-8f32));
    pwasm_ethereum::ret(&b"success"[..]);
}
