<<<<<<< HEAD
// #[macro_use]
// extern crate tvm;

extern crate ndarray;
extern crate pwasm_ethereum;
=======
extern crate ndarray;
#[macro_use]
extern crate tvm;

extern crate pwasm_ethereum;
extern crate pwasm_std;

use std::convert::TryInto;

use ndarray::Array;
use pwasm_std::logger::debug;
use tvm::{
    ffi::runtime::DLTensor,
    runtime::{Module, SystemLibModule},
};
>>>>>>> 63d41c3... Setup integration test

#[no_mangle]
pub fn deploy() {}

#[no_mangle]
pub fn call() {
<<<<<<< HEAD
	let dataset = vec![vec![0.0, 5.1, 3.5, 1.4, 0.2],
                       vec![0.0, 4.9, 3.0, 1.4, 0.2],
                       vec![0.0, 4.7, 3.2, 1.3, 0.2],
                       vec![0.0, 4.6, 3.1, 1.5, 0.2],
                       vec![0.0, 5.0, 3.6, 1.4, 0.2],
                       vec![0.0, 5.4, 3.9, 1.7, 0.4],
                       vec![0.0, 4.6, 3.4, 1.4, 0.3],
                       vec![0.0, 5.0, 3.4, 1.5, 0.2],
                       vec![0.0, 4.4, 2.9, 1.4, 0.2],
                       vec![0.0, 4.9, 3.1, 1.5, 0.1],
                       vec![0.0, 5.4, 3.7, 1.5, 0.2],
                       vec![0.0, 4.8, 3.4, 1.6, 0.2],
                       vec![0.0, 4.8, 3.0, 1.4, 0.1],
                       vec![0.0, 4.3, 3.0, 1.1, 0.1],
                       vec![0.0, 5.8, 4.0, 1.2, 0.2],
                       vec![0.0, 5.7, 4.4, 1.5, 0.4],
                       vec![0.0, 5.4, 3.9, 1.3, 0.4],
                       vec![0.0, 5.1, 3.5, 1.4, 0.3],
                       vec![0.0, 5.7, 3.8, 1.7, 0.3],
                       vec![0.0, 5.1, 3.8, 1.5, 0.3],
                       vec![0.0, 5.4, 3.4, 1.7, 0.2],
                       vec![0.0, 5.1, 3.7, 1.5, 0.4],
                       vec![0.0, 4.6, 3.6, 1.0, 0.2],
                       vec![0.0, 5.1, 3.3, 1.7, 0.5],
                       vec![0.0, 4.8, 3.4, 1.9, 0.2],
                       vec![0.0, 5.0, 3.0, 1.6, 0.2],
                       vec![0.0, 5.0, 3.4, 1.6, 0.4],
                       vec![0.0, 5.2, 3.5, 1.5, 0.2],
                       vec![0.0, 5.2, 3.4, 1.4, 0.2],
                       vec![0.0, 4.7, 3.2, 1.6, 0.2],
                       vec![0.0, 4.8, 3.1, 1.6, 0.2],
                       vec![0.0, 5.4, 3.4, 1.5, 0.4],
                       vec![0.0, 5.2, 4.1, 1.5, 0.1],
                       vec![0.0, 5.5, 4.2, 1.4, 0.2],
                       vec![0.0, 4.9, 3.1, 1.5, 0.1],
                       vec![0.0, 5.0, 3.2, 1.2, 0.2],
                       vec![0.0, 5.5, 3.5, 1.3, 0.2],
                       vec![0.0, 4.9, 3.1, 1.5, 0.1],
                       vec![0.0, 4.4, 3.0, 1.3, 0.2],
                       vec![0.0, 5.1, 3.4, 1.5, 0.2],
                       vec![0.0, 5.0, 3.5, 1.3, 0.3],
                       vec![0.0, 4.5, 2.3, 1.3, 0.3],
                       vec![0.0, 4.4, 3.2, 1.3, 0.2],
                       vec![0.0, 5.0, 3.5, 1.6, 0.6],
                       vec![0.0, 5.1, 3.8, 1.9, 0.4],
                       vec![0.0, 4.8, 3.0, 1.4, 0.3],
                       vec![0.0, 5.1, 3.8, 1.6, 0.2],
                       vec![0.0, 4.6, 3.2, 1.4, 0.2],
                       vec![0.0, 5.3, 3.7, 1.5, 0.2],
                       vec![0.0, 5.0, 3.3, 1.4, 0.2],
                       vec![1.0, 7.0, 3.2, 4.7, 1.4],
                       vec![1.0, 6.4, 3.2, 4.5, 1.5],
                       vec![1.0, 6.9, 3.1, 4.9, 1.5],
                       vec![1.0, 5.5, 2.3, 4.0, 1.3],
                       vec![1.0, 6.5, 2.8, 4.6, 1.5],
                       vec![1.0, 5.7, 2.8, 4.5, 1.3],
                       vec![1.0, 6.3, 3.3, 4.7, 1.6],
                       vec![1.0, 4.9, 2.4, 3.3, 1.0],
                       vec![1.0, 6.6, 2.9, 4.6, 1.3],
                       vec![1.0, 5.2, 2.7, 3.9, 1.4],
                       vec![1.0, 5.0, 2.0, 3.5, 1.0],
                       vec![1.0, 5.9, 3.0, 4.2, 1.5],
                       vec![1.0, 6.0, 2.2, 4.0, 1.0],
                       vec![1.0, 6.1, 2.9, 4.7, 1.4],
                       vec![1.0, 5.6, 2.9, 3.6, 1.3],
                       vec![1.0, 6.7, 3.1, 4.4, 1.4],
                       vec![1.0, 5.6, 3.0, 4.5, 1.5],
                       vec![1.0, 5.8, 2.7, 4.1, 1.0],
                       vec![1.0, 6.2, 2.2, 4.5, 1.5],
                       vec![1.0, 5.6, 2.5, 3.9, 1.1],
                       vec![1.0, 5.9, 3.2, 4.8, 1.8],
                       vec![1.0, 6.1, 2.8, 4.0, 1.3],
                       vec![1.0, 6.3, 2.5, 4.9, 1.5],
                       vec![1.0, 6.1, 2.8, 4.7, 1.2],
                       vec![1.0, 6.4, 2.9, 4.3, 1.3],
                       vec![1.0, 6.6, 3.0, 4.4, 1.4],
                       vec![1.0, 6.8, 2.8, 4.8, 1.4],
                       vec![1.0, 6.7, 3.0, 5.0, 1.7],
                       vec![1.0, 6.0, 2.9, 4.5, 1.5],
                       vec![1.0, 5.7, 2.6, 3.5, 1.0],
                       vec![1.0, 5.5, 2.4, 3.8, 1.1],
                       vec![1.0, 5.5, 2.4, 3.7, 1.0],
                       vec![1.0, 5.8, 2.7, 3.9, 1.2],
                       vec![1.0, 6.0, 2.7, 5.1, 1.6],
                       vec![1.0, 5.4, 3.0, 4.5, 1.5],
                       vec![1.0, 6.0, 3.4, 4.5, 1.6],
                       vec![1.0, 6.7, 3.1, 4.7, 1.5],
                       vec![1.0, 6.3, 2.3, 4.4, 1.3],
                       vec![1.0, 5.6, 3.0, 4.1, 1.3],
                       vec![1.0, 5.5, 2.5, 4.0, 1.3],
                       vec![1.0, 5.5, 2.6, 4.4, 1.2],
                       vec![1.0, 6.1, 3.0, 4.6, 1.4],
                       vec![1.0, 5.8, 2.6, 4.0, 1.2],
                       vec![1.0, 5.0, 2.3, 3.3, 1.0],
                       vec![1.0, 5.6, 2.7, 4.2, 1.3],
                       vec![1.0, 5.7, 3.0, 4.2, 1.2],
                       vec![1.0, 5.7, 2.9, 4.2, 1.3],
                       vec![1.0, 6.2, 2.9, 4.3, 1.3],
                       vec![1.0, 5.1, 2.5, 3.0, 1.1],
                       vec![1.0, 5.7, 2.8, 4.1, 1.3]
                    ];
	let mut a_nd: ndarray::Array = ndarray::Array::from_vec(&dataset);
	println!("{:?}", a_nd);
	// let mut a: Tensor = a_nd.into();

	// let mut a_dl: DLTensor = (&mut t).into();
	// call_packed!(tvm_fn, &mut a_dl);

	// let mut a_nd = ndarray::Array::try_from(&a).unwrap();
}
=======
    // Must be run first to register the functions.
    unsafe { __tvm_module_startup(); }

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
>>>>>>> 63d41c3... Setup integration test
