#!/usr/bin/env python3

import os
import sys
import tvm

def prepare_test_libs(base_path):
    # The target is very important and is what allows the resulting module
    # to be linked to the Wasm contract.
    target = "llvm -target=wasm32-unknown-unknown-wasm -system-lib"
    if not tvm.module.enabled(target):
        raise RuntimeError("Target %s is not enabled" % target)
    n = tvm.var("n")
    A = tvm.placeholder((n,), name='A')
    B = tvm.compute(A.shape, lambda *i: A(*i) + 1.0, name='B')
    s = tvm.create_schedule(B.op)
    fadd1 = tvm.build(s, [A, B], target, name="add_one")
    obj_path = os.path.join(base_path, "test_add_one.o")
    fadd1.save(obj_path)

if __name__ == "__main__":
    prepare_test_libs(sys.argv[1])
