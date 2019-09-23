#!/usr/bin/python
import json
import sys

colors = {
    'trace': '35',
    'debug': '34',
    'info': '32',
    'warn': '33',
    'error': '31',
    # https://github.com/slog-rs/slog/blob/v2.4.1/src/lib.rs#L2057
    'TRCE': '35;1',
    'DEBG': '34;1',
    'INFO': '32;1',
    'WARN': '33;1',
    'ERRO': '31;1',
    'CRIT': '31;1',
}

omitkeys = set(['ts', 'level', 'module', 'msg'])

max_module_len = 0

while True:
    line = sys.stdin.readline()
    if not line:
        break
    if line[0] != '{':
        print line,
        continue
    try:
        record = json.loads(line)
    except ValueError as e:
        print '\033[35m%s\033[0m' % e, line,
        continue
    level = record.get('level', '')
    color = colors.get(level, '31')
    module = record.get('module', '')
    max_module_len = max(max_module_len, len(module))
    msg = record.get('msg', '')
    kvs = ''.join(' \033[33m%s\033[0m=%s' % (key, json.dumps(record[key])) for key in sorted(set(record.keys()) - omitkeys))
    print ' \033[%sm%-5s\033[0m \033[1m%-*s\33[0m > %s%s' % (color, level, max_module_len, module, msg, kvs)
