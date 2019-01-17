extern crate hex;

extern crate rand;
#[macro_use]
extern crate rulinalg;
extern crate dpmlrust;
extern crate ml_reader;
extern crate num as libnum;
extern crate owasm_std;
extern crate rusty_libsvm;
extern crate rusty_machine;

use std::panic;
use std::str;

use std::fmt::Debug;
use std::fs;
use std::io::{Read, Write};
use std::mem;
use std::path;
use std::path::PathBuf;
use std::vec::Vec;

use dpmlrust::logistic::{accuracy, add_normal_noise, compute_grad, learn, predict, update_model};
use ml_reader::rusty::Dataset;
use ml_reader::rusty::Reader;
use owasm_std::logger::debug;
use rulinalg::matrix::{Axes, BaseMatrix, BaseMatrixMut, Matrix, MatrixSlice, MatrixSliceMut};
use rulinalg::norm;
use rulinalg::vector::Vector;
use rusty_libsvm::Libsvm;

#[owasm_abi_derive::contract]
trait Logistic {
    fn constructor(&mut self) {}

    fn regression(&mut self) -> Vec<u8> {
        panic::set_hook(Box::new(|panic_info| {
            if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
                owasm_std::logger::debug(s);
            }
        }));

        debug("In call...");

        // dataset in a buffer for now
        let buffer = "0	0:5.1 1:3.5 2:1.4 3:0.2
                          0	0:4.9 1:3.0 2:1.4 3:0.2
                          0	0:4.7 1:3.2 2:1.3 3:0.2
                          0	0:4.6 1:3.1 2:1.5 3:0.2
                          0	0:5.0 1:3.6 2:1.4 3:0.2
                          0	0:5.4 1:3.9 2:1.7 3:0.4
                          0	0:4.6 1:3.4 2:1.4 3:0.3
                          0	0:5.0 1:3.4 2:1.5 3:0.2
                          0	0:4.4 1:2.9 2:1.4 3:0.2
                          0	0:4.9 1:3.1 2:1.5 3:0.1
                          0	0:5.4 1:3.7 2:1.5 3:0.2
                          0	0:4.8 1:3.4 2:1.6 3:0.2
                          0	0:4.8 1:3.0 2:1.4 3:0.1
                          0	0:4.3 1:3.0 2:1.1 3:0.1
                          0	0:5.8 1:4.0 2:1.2 3:0.2
                          0	0:5.7 1:4.4 2:1.5 3:0.4
                          0	0:5.4 1:3.9 2:1.3 3:0.4
                          0	0:5.1 1:3.5 2:1.4 3:0.3
                          0	0:5.7 1:3.8 2:1.7 3:0.3
                          0	0:5.1 1:3.8 2:1.5 3:0.3
                          0	0:5.4 1:3.4 2:1.7 3:0.2
                          0	0:5.1 1:3.7 2:1.5 3:0.4
                          0	0:4.6 1:3.6 2:1.0 3:0.2
                          0	0:5.1 1:3.3 2:1.7 3:0.5
                          0	0:4.8 1:3.4 2:1.9 3:0.2
                          0	0:5.0 1:3.0 2:1.6 3:0.2
                          0	0:5.0 1:3.4 2:1.6 3:0.4
                          0	0:5.2 1:3.5 2:1.5 3:0.2
                          0	0:5.2 1:3.4 2:1.4 3:0.2
                          0	0:4.7 1:3.2 2:1.6 3:0.2
                          0	0:4.8 1:3.1 2:1.6 3:0.2
                          0	0:5.4 1:3.4 2:1.5 3:0.4
                          0	0:5.2 1:4.1 2:1.5 3:0.1
                          0	0:5.5 1:4.2 2:1.4 3:0.2
                          0	0:4.9 1:3.1 2:1.5 3:0.1
                          0	0:5.0 1:3.2 2:1.2 3:0.2
                          0	0:5.5 1:3.5 2:1.3 3:0.2
                          0	0:4.9 1:3.1 2:1.5 3:0.1
                          0	0:4.4 1:3.0 2:1.3 3:0.2
                          0	0:5.1 1:3.4 2:1.5 3:0.2
                          0	0:5.0 1:3.5 2:1.3 3:0.3
                          0	0:4.5 1:2.3 2:1.3 3:0.3
                          0	0:4.4 1:3.2 2:1.3 3:0.2
                          0	0:5.0 1:3.5 2:1.6 3:0.6
                          0	0:5.1 1:3.8 2:1.9 3:0.4
                          0	0:4.8 1:3.0 2:1.4 3:0.3
                          0	0:5.1 1:3.8 2:1.6 3:0.2
                          0	0:4.6 1:3.2 2:1.4 3:0.2
                          0	0:5.3 1:3.7 2:1.5 3:0.2
                          0	0:5.0 1:3.3 2:1.4 3:0.2
                          1	0:7.0 1:3.2 2:4.7 3:1.4
                          1	0:6.4 1:3.2 2:4.5 3:1.5
                          1	0:6.9 1:3.1 2:4.9 3:1.5
                          1	0:5.5 1:2.3 2:4.0 3:1.3
                          1	0:6.5 1:2.8 2:4.6 3:1.5
                          1	0:5.7 1:2.8 2:4.5 3:1.3
                          1	0:6.3 1:3.3 2:4.7 3:1.6
                          1	0:4.9 1:2.4 2:3.3 3:1.0
                          1	0:6.6 1:2.9 2:4.6 3:1.3
                          1	0:5.2 1:2.7 2:3.9 3:1.4
                          1	0:5.0 1:2.0 2:3.5 3:1.0
                          1	0:5.9 1:3.0 2:4.2 3:1.5
                          1	0:6.0 1:2.2 2:4.0 3:1.0
                          1	0:6.1 1:2.9 2:4.7 3:1.4
                          1	0:5.6 1:2.9 2:3.6 3:1.3
                          1	0:6.7 1:3.1 2:4.4 3:1.4
                          1	0:5.6 1:3.0 2:4.5 3:1.5
                          1	0:5.8 1:2.7 2:4.1 3:1.0
                          1	0:6.2 1:2.2 2:4.5 3:1.5
                          1	0:5.6 1:2.5 2:3.9 3:1.1
                          1	0:5.9 1:3.2 2:4.8 3:1.8
                          1	0:6.1 1:2.8 2:4.0 3:1.3
                          1	0:6.3 1:2.5 2:4.9 3:1.5
                          1	0:6.1 1:2.8 2:4.7 3:1.2
                          1	0:6.4 1:2.9 2:4.3 3:1.3
                          1	0:6.6 1:3.0 2:4.4 3:1.4
                          1	0:6.8 1:2.8 2:4.8 3:1.4
                          1	0:6.7 1:3.0 2:5.0 3:1.7
                          1	0:6.0 1:2.9 2:4.5 3:1.5
                          1	0:5.7 1:2.6 2:3.5 3:1.0
                          1	0:5.5 1:2.4 2:3.8 3:1.1
                          1	0:5.5 1:2.4 2:3.7 3:1.0
                          1	0:5.8 1:2.7 2:3.9 3:1.2
                          1	0:6.0 1:2.7 2:5.1 3:1.6
                          1	0:5.4 1:3.0 2:4.5 3:1.5
                          1	0:6.0 1:3.4 2:4.5 3:1.6
                          1	0:6.7 1:3.1 2:4.7 3:1.5
                          1	0:6.3 1:2.3 2:4.4 3:1.3
                          1	0:5.6 1:3.0 2:4.1 3:1.3
                          1	0:5.5 1:2.5 2:4.0 3:1.3
                          1	0:5.5 1:2.6 2:4.4 3:1.2
                          1	0:6.1 1:3.0 2:4.6 3:1.4
                          1	0:5.8 1:2.6 2:4.0 3:1.2
                          1	0:5.0 1:2.3 2:3.3 3:1.0
                          1	0:5.6 1:2.7 2:4.2 3:1.3
                          1	0:5.7 1:3.0 2:4.2 3:1.2
                          1	0:5.7 1:2.9 2:4.2 3:1.3
                          1	0:6.2 1:2.9 2:4.3 3:1.3
                          1	0:5.1 1:2.5 2:3.0 3:1.1
                          1	0:5.7 1:2.8 2:4.1 3:1.3
                          ";

        debug("About to read from buffer");

        let mut dataset = Libsvm::read_from_buffer(&buffer.to_string(), false, 4);

        debug("Done reading from buffer");

        debug(&format!("{}", dataset.data()));
        debug(&format!("{:?}", dataset.target()));

        let model = learn(&dataset, 1);

        let raw_samples = dataset.data();
        let ones = Matrix::<f64>::ones(raw_samples.rows(), 1);
        let samples = ones.hcat(raw_samples);
        let targets = dataset.target();
        let sample_num = samples.rows();
        let feature_num = samples.cols();

        let mut result = predict(model, samples);

        for i in result.mut_data().into_iter() {
            debug(&format!("{}", i));
        }
        let classes = result
            .into_iter()
            .map(|x| {
                if x > 0.5 {
                    return 1.0;
                } else {
                    return 0.0;
                }
            })
            .collect::<Vec<_>>();
        let matching = classes
            .into_iter()
            .zip(targets.into_iter())
            .filter(|(a, b)| a == *b)
            .count();

        let result = format!("Matching classes is {}", matching);
        debug(&result);
        result.as_bytes().to_vec()
    }
}
