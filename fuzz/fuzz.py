#!/usr/bin/env python3

import argparse
import subprocess
import os
import os.path as osp

KM_ENV = 'KM_ENCLAVE_PATH'
REPO_ROOT = osp.abspath(osp.join(osp.dirname(__file__), '..'))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('target', help='The name of the fuzz target to run')
    args = parser.parse_args()
    run(f'cargo hfuzz run {args.target}')


def run(cmd):
    env = dict(os.environ)
    if KM_ENV not in env:
        env[KM_ENV] = osp.join(REPO_ROOT, '.ekiden', 'target',
                               'x86_64-fortanix-unknown-sgx', 'debug',
                               'ekiden-keymanager-runtime.sgxs')
    subprocess.run(cmd, env=env, cwd=REPO_ROOT, shell=True, check=True)


if __name__ == '__main__':
    main()
