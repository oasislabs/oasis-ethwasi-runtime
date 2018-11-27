#!/usr/bin/env python3

from os import path as osp
import sys

import nnvm
from nnvm.compiler import optimizer, graph_util
from nnvm.testing import init
import numpy as np
import tvm

def log_loss(model, y):
    diff = nnvm.symbol.elemwise_add(
        nnvm.symbol.elemwise_mul(y, nnvm.symbol.log(model)),
        nnvm.symbol.elemwise_mul(1 - y, nnvm.symbol.log(1 - model)))
    return nnvm.symbol.negative(diff)

def get_weights(model):
    weights = []
    for var in model.list_input_variables():
        name = var.attr('name')
        if name == 'data':
            continue
        var_type = name.rsplit('_', 1)[-1]
        if var_type not in {'gamma', 'beta', 'weight', 'bias'}:
            continue
        weights.append(var)
    return weights

def init_params(graph, input_shapes, initializer=init.Xavier(), seed=10):
    ishapes, _ = graph_util.infer_shape(graph, **input_shapes)
    param_shapes = dict(zip(graph.index.input_names, ishapes))
    np.random.seed(seed)
    params = {}
    for param, shape in param_shapes.items():
        if param == 'data' or param == 'y' or not shape:
            continue
        init_value = np.empty(shape).astype('float32')
        print("param:" + param)
        initializer(param, init_value)
        params[param] = tvm.nd.array(init_value)
    return params

def main():
    batch_size = 1
    data_var = nnvm.symbol.Variable(name='data', shape=(batch_size,23), dtype="float32")
    y_var = nnvm.symbol.Variable(name='y', shape=(batch_size,1), dtype="float32")

    model = nnvm.symbol.dense(data=data_var, units=1)
    model = nnvm.symbol.sigmoid(model)

    loss = log_loss(model, y_var)
    base_lr = 0.1
    optim = optimizer.SGD(learning_rate=base_lr,
                          lr_scheduler=None,
                          clip_gradient=None,
                          wd=0)
    optim = optim.minimize(loss, var=get_weights(model))
    compute_graph = nnvm.graph.create(optim)

    ishapes, _ = graph_util.infer_shape(compute_graph)
    ishape_dict = dict(zip(compute_graph.index.input_names, ishapes))
    params = init_params(compute_graph, ishape_dict)
    # print(compute_graph.ir())

    deploy_graph, lib, params = nnvm.compiler.build(compute_graph,
                                                    target='llvm -target=wasm32 -system-lib',
                                                    shape=ishape_dict,
                                                    params=params,
                                                    dtype='float32')
    params.pop('SGD_t', None)

    out_dir = sys.argv[1]
    lib.save(osp.join(out_dir, "credit_scoring.o"))
    with open(osp.join(out_dir, "graph.json"), 'w') as f_graph:
        f_graph.write(deploy_graph.json())
    with open(osp.join(out_dir, "graph.params"), 'wb') as f_params:
        f_params.write(nnvm.compiler.save_param_dict(params))

if __name__ == "__main__":
    main()
