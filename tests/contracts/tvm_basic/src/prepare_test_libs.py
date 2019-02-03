#!/usr/bin/env python3

"""Creates a simple TVM module."""

import argparse
from os import path as osp

import tvm


N = 16
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('-o', '--out-dir', default='.')
    args = parser.parse_args()

    dshape = (N,)
    a = tvm.placeholder(dshape, name='A', dtype='int8')
    b = tvm.placeholder(dshape, name='B', dtype='int8')
    c = tvm.compute(dshape, lambda *i: a(*i) + b(*i), name='C')
    s = tvm.create_schedule(c.op)
    add = tvm.build(s, [a, b, c],
                    'llvm -target=wasm32-unknown-unknown-wasm -system-lib',
                    name='add')
    add.save(osp.join(args.out_dir, 'add.o'))


if __name__ == '__main__':
    main()
