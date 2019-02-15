#!/usr/bin/python
import collections
import json
import re
import sys

#    name          color     scale
OPS_INFO = [
    ('get',        0x2196F3,    1.),
    ('set',        0xF44336,    1.),
    ('tx-put',     0xFFC107,    1.),
    ('dh-insert',  0xFF9800,    1.),
    ('sh-set',     0x9C27B0,    1.),
    ('tr-alloc',   0xCDDC39,    1.),
    ('st-insert',  0x4CAF50,    1.),
    ('charge-gas', 0x009688,  256.),
]
OPS = [op for op, color, scale in OPS_INFO]
def new_counts():
    return dict((op, 0) for op in OPS)

def summarize_counts(counts):
    return ' / '.join('%d %s' % (counts[op], op) for op in OPS)

def scale_counts(counts):
    return dict((op, counts[op] / scale) for op, color, scale in OPS_INFO)

def total_counts(counts):
    return sum(counts[op] for op in OPS)

def color_counts(counts, total):
    r = int(sum((color >> 16 & 0xff) * counts[op] for op, color, scale in OPS_INFO) / total)
    g = int(sum((color >>  8 & 0xff) * counts[op] for op, color, scale in OPS_INFO) / total)
    b = int(sum((color >>  0 & 0xff) * counts[op] for op, color, scale in OPS_INFO) / total)
    return '#%02x%02x%02x' % (r, g, b)

WANT_ERA = 'bulk-storage'

era_keep = False
ips = {}
verts = collections.defaultdict(new_counts)
edges = collections.defaultdict(new_counts)
ips_by_label = {}

def maybe_print_edge(src, dst):
    if src not in ips_by_label:
        print >>sys.stderr, 'warning: didn\'t encounter label %s' % src
        return
    if dst not in ips_by_label:
        print >>sys.stderr, 'warning: didn\'t encounter label %s' % dst
        return
    print '  "0x%016x" -> "0x%016x" [style=dashed]' % (
        ips_by_label[src],
        ips_by_label[dst],
    )

for line in sys.stdin:
    m = re.match(r'\[storagestudy\] (\S+) (.*)$', line)
    if m is None:
        continue
    ev = m.group(1)
    args = m.group(2)
    if ev == 'new-ip':
        ip = json.loads(args)
        ips[ip] = []
    elif ev == 'ip-symbol':
        m = re.match(r'(\d+) (.*)$', args)
        ip = json.loads(m.group(1))
        symbol = m.group(2)
        ips[ip].append(symbol)
    elif ev == 'era':
        print >>sys.stderr, 'era %s' % args
        era_keep = args == WANT_ERA
    else:
        detail = ''
        m = re.match(r'(.*?)\((.*)\)$', ev)
        if m is not None:
            ev = m.group(1)
            detail = m.group(2)
        if ev in OPS:
            if not era_keep:
                continue
            weight = int(detail) if detail else 1
            chain = json.loads(args)
            # strip some junk
            chain = chain[3:-12]
            for ip in chain:
                verts[ip][ev] += weight
            for i in range(len(chain) - 1):
                ip_callee = chain[i]
                ip_caller = chain[i + 1]
                edges[(ip_caller, ip_callee)][ev] += weight

print 'digraph storagestudy {'
print '  node [shape=box]'

for ip, counts in verts.iteritems():
    scaled_counts = scale_counts(counts)
    scaled_total = total_counts(scaled_counts)
    label = None
    for symbol in ips[ip]:
        simple_symbol = symbol
        while True:
            simpler = simple_symbol
            simpler = re.sub(r'<([^<>]*) as [^<>]*>', r'\1', simpler)
            simpler = re.sub(r'<impl [^<>]* for ([^<>]*)>', r'\1', simpler)
            simpler = re.sub(r'<([^<>]*)>::', r'\1::', simpler)
            simpler = re.sub(r'<[^<>]*>', r'', simpler)
            if simpler == simple_symbol:
                break
            simple_symbol = simpler
        simple_symbol = re.sub(r'::h[0-9a-f]{16}', r'', simple_symbol)
        m = re.search(r'name: (.+?),? ', simple_symbol)
        if m is not None:
            label = m.group(1)
            label = label.replace('"', '')
            parts = label.split('::')
            max_parts = 3 if parts[-1] == '{{closure}}' else 2
            if len(parts) > max_parts:
                label = '::'.join(parts[-max_parts:])
            m = re.search(r'lineno: (\d+)', symbol)
            if m is not None:
                label += ':%s' % m.group(1)
            ips_by_label[label] = ip
            break
    if label is None:
        label = '0x%016x' % ip
    symbols_escaped = '\\n'.join(symbol.replace('\\', '\\\\').replace('"', '\\"') for symbol in ips[ip])
    print '  "0x%016x" [label="%s", tooltip="%s\\n%s", color="%s"]' % (
        ip,
        label,
        summarize_counts(counts),
        symbols_escaped,
        color_counts(scaled_counts, scaled_total),
    )

for (ip_caller, ip_callee), counts in edges.iteritems():
    scaled_counts = scale_counts(counts)
    scaled_total = total_counts(scaled_counts)
    size = scaled_total ** (1. / 3)
    print '  "0x%016x" -> "0x%016x" [tooltip="%s", penwidth=%f, color="%s"]' % (
        ip_caller,
        ip_callee,
        summarize_counts(counts),
        size,
        color_counts(scaled_counts, scaled_total),
    )

maybe_print_edge('State::insert_cache:534', 'State::commit:932')
maybe_print_edge('NodeStorage::alloc:246', 'TrieDBMut::commit_node:872')
maybe_print_edge('NodeStorage::alloc:246', 'TrieDBMut::commit:848')
maybe_print_edge('StorageHashDB::insert:256', 'StorageHashDB::commit:128')

maybe_print_edge('DBTransaction::put:101', 'BlockchainStateDb::write_buffered:376')
maybe_print_edge('DatabaseHandle::insert:293', 'DatabaseHandle::commit:343')

print '}'
