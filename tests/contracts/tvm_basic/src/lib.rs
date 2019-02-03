#![feature(try_from)]

#[macro_use]
extern crate tvm;

use std::convert::TryInto;

use ndarray::Array;
use tvm::{ffi::runtime::DLTensor,
          runtime::{Module, SystemLibModule}};

extern "C" {
    fn __tvm_module_startup();
}

#[no_mangle]
pub fn deploy() {}

#[no_mangle]
pub fn call() {
    unsafe {
        __tvm_module_startup();
    }

    let syslib = SystemLibModule::default();
    let add_one = syslib.get_function("add").unwrap();
    let mut a = Array::from_vec(vec![1f32, 1., 2., 3.]);
    let mut b = Array::from_vec(vec![5f32, 8., 13., 21.]);
    let mut a_dl: DLTensor = (&mut a).into();
    let mut b_dl: DLTensor = (&mut b).into();

    let c = Array::from_vec(vec![6f32, 9., 15., 24.]);

    let _result: i32 = call_packed!(add_one, &mut a_dl, &mut b_dl)
        .try_into()
        .unwrap();
    assert!(c.all_close(&b, 1e-8f32));
    owasm_ethereum::ret(&b"success"[..]);
}
