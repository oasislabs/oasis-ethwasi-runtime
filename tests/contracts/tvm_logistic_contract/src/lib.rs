#![feature(try_from)]

extern crate ml_reader;
#[macro_use]
extern crate ndarray;
#[macro_use]
extern crate tvm;
extern crate tvm_libsvm;

extern crate pwasm_ethereum;
extern crate pwasm_std;

use std::convert::{TryFrom, TryInto};
use std::panic;

use ml_reader::tvm::{Dataset, TVMReader};
use ndarray::Array2;
use pwasm_std::logger::debug;
use tvm::runtime::{Graph, GraphExecutor, Module, SystemLibModule};
use tvm_libsvm::TVMLibsvm;

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
    panic::set_hook(Box::new(|panic_info| println!("{}", panic_info)));
    // Must be run first to register the functions.
    unsafe {
        __tvm_module_startup();
    }

    let input = include_bytes!("../data/training.data");
    let graph_json = include_str!("../tvm_module/graph.json");
    let graph_params = include_bytes!("../tvm_module/graph.params");
    let graph = Graph::try_from(graph_json).unwrap();
    //println!("graph: {:?}", graph);
    let params = tvm::runtime::load_param_dict(graph_params).unwrap();
    let syslib = SystemLibModule::default();
    let mut exec = GraphExecutor::new(graph, &syslib).unwrap();
    let dataset = TVMLibsvm::read_byte_array(input, 23);

    exec.load_params(params);
    println!("{:?}", dataset.data.slice(s![..1, ..]).shape());
    let data: Array2<f32> = dataset.data.slice(s![..1, ..]).to_owned();
    exec.set_input("data", data.into());
    //exec.set_input("y", dataset.label);
    //exec.run();

    //let weights = exec.get_output(0).unwrap();
    pwasm_ethereum::ret(&b"success"[..]);
}
