#!/usr/bin/env python3
"""Measure explicit NITF preparation; baseline unless a separate freeze is supplied."""
import argparse
import ctypes
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import stat
import subprocess
import sys
import time


READS = {'read', 'pread64', 'readv', 'preadv', 'preadv2'}
TRANSFERS = {'sendfile', 'splice', 'copy_file_range'}
TRACE_CALLS = sorted(READS | TRANSFERS | {'mmap', 'io_uring_setup'})
ENV_KEYS = ('PATH', 'LD_LIBRARY_PATH', 'LANG', 'LC_ALL', 'GDAL_DATA', 'PROJ_DATA')
CACHE_WINDOW_BYTES = 64 << 20


def cache_file_stat(fd):
    info = os.fstat(fd)
    if not stat.S_ISREG(info.st_mode) or info.st_size <= 0:
        raise ValueError('source cache observation requires a non-empty regular file')
    # Linux can return all-resident masks to callers without sufficient file
    # authority. Require ownership instead of treating that mask as warm proof.
    if info.st_uid != os.geteuid():
        raise ValueError('source cache observation requires caller-owned authored input')
    return dict(device=info.st_dev, inode=info.st_ino, bytes=info.st_size,
                mtime_ns=info.st_mtime_ns, ctime_ns=info.st_ctime_ns)


def source_residency(path):
    """Snapshot page-cache residency without touching source mapping contents.

    At most 64 MiB of virtual address space and one byte per page are used at
    once. Every PROT_NONE mapping ends before returning to the caller.
    """
    started = time.monotonic_ns()
    library = ctypes.CDLL(None, use_errno=True)
    library.mmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int,
                            ctypes.c_int, ctypes.c_int, ctypes.c_long]
    library.mmap.restype = ctypes.c_void_p
    library.mincore.argtypes = [ctypes.c_void_p, ctypes.c_size_t,
                               ctypes.POINTER(ctypes.c_ubyte)]
    library.mincore.restype = ctypes.c_int
    library.munmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    library.munmap.restype = ctypes.c_int
    page_size = os.sysconf('SC_PAGE_SIZE')
    window = max(page_size, CACHE_WINDOW_BYTES // page_size * page_size)
    resident, pages, windows = 0, 0, 0
    with Path(path).open('rb', buffering=0) as stream:
        identity = cache_file_stat(stream.fileno())
        for offset in range(0, identity['bytes'], window):
            length = min(window, identity['bytes'] - offset)
            count = (length + page_size - 1) // page_size
            vector = (ctypes.c_ubyte * count)()
            address = library.mmap(None, length, 0, 1, stream.fileno(), offset)
            if address == ctypes.c_void_p(-1).value:
                raise OSError(ctypes.get_errno(), 'source residency mmap failed')
            try:
                if library.mincore(address, length, vector):
                    raise OSError(ctypes.get_errno(), 'source residency mincore failed')
                resident += sum(value & 1 for value in vector)
                pages += count
                windows += 1
            finally:
                if library.munmap(address, length):
                    raise OSError(ctypes.get_errno(), 'source residency munmap failed')
        if cache_file_stat(stream.fileno()) != identity:
            raise ValueError('source changed during residency observation')
    return dict(available=True, file=identity, page_size=page_size, pages=pages,
                resident_pages=resident, nonresident_pages=pages - resident,
                mapping_windows=windows, maximum_mapping_bytes=window,
                sampled_started_monotonic_ns=started,
                sampled_finished_monotonic_ns=time.monotonic_ns())


def condition_source_cache(path, state, evidence):
    """Condition only this file, after identity hashing and before measurement."""
    evidence.update(requested_state=state, conditioning_read_bytes=0,
                    conditioning_read_operations=0, conditioning_ms=0)
    started = time.monotonic_ns()
    if state != 'uncontrolled':
        with Path(path).open('rb', buffering=0) as stream:
            identity = cache_file_stat(stream.fileno())
            if state == 'cold-os':
                os.fsync(stream.fileno())
                page_size = os.sysconf('SC_PAGE_SIZE')
                length = (identity['bytes'] + page_size - 1) // page_size * page_size
                os.posix_fadvise(stream.fileno(), 0, length, os.POSIX_FADV_DONTNEED)
                evidence['method'] = 'file fsync then page-aligned POSIX_FADV_DONTNEED'
                evidence['advised_bytes'] = length
            elif state == 'warm-os':
                while chunk := stream.read(1 << 20):
                    evidence['conditioning_read_bytes'] += len(chunk)
                    evidence['conditioning_read_operations'] += 1
                evidence['method'] = 'explicit sequential read with at most 1 MiB per buffer'
                if evidence['conditioning_read_bytes'] != identity['bytes']:
                    raise ValueError('source changed during warm-cache pass')
            else:
                raise ValueError('unsupported source cache state')
            if cache_file_stat(stream.fileno()) != identity:
                raise ValueError('source changed during cache conditioning')
    else:
        evidence['method'] = 'no conditioning after wrapper identity hash pass'
    evidence['conditioning_ms'] = (time.monotonic_ns() - started) / 1e6
    observe_source_cache(path, 'before', state, evidence)
    before = evidence['before']
    if state == 'cold-os' and before['resident_pages'] != 0:
        raise ValueError('cold-os admission failed: source pages remain resident')
    if state == 'warm-os' and before['resident_pages'] != before['pages']:
        raise ValueError('warm-os admission failed: not every source page is resident')


def observe_source_cache(path, phase, state, evidence):
    try:
        evidence[phase] = source_residency(path)
    except (OSError, ValueError, AttributeError) as error:
        evidence[phase] = dict(available=False, error=str(error))
        if state != 'uncontrolled':
            raise ValueError(f'{state} source residency unavailable: {error}') from error


def digest(path):
    value = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b''):
            value.update(chunk)
    return value.hexdigest()


def file_identity(path):
    path = Path(path).resolve(strict=True)
    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': digest(path)}


def safe_trace_path(path):
    """Admit only paths that strace -yy prints literally and unambiguously."""
    value = str(Path(path).resolve(strict=True))
    if not value.isascii() or any(ord(c) < 32 or ord(c) > 126 or c in '<>\\"' for c in value):
        raise ValueError('source/library paths must be literal printable ASCII for trace attribution')
    return value


def trace_command(strace, trace_prefix, command):
    # -s 0 omits buffer contents (including iovec buffers), -ff gives one file per
    # traced task and -yy resolves descriptor paths at the actual syscall.
    return [str(strace), '-ff', '-yy', '-s', '0', '-o', str(trace_prefix),
            '-e', 'trace=' + ','.join(TRACE_CALLS), '--', *command]


def parse_source_trace(paths, source):
    """Fail closed on incomplete traces, unknown reads or alternative source I/O.

    Returned bytes count successful syscalls, including rereads and the tool's
    deliberate hash scan. This is neither device traffic nor a GDAL callback.
    """
    source = safe_trace_path(source)
    counts = dict(read_bytes=0, read_operations=0, zero_read_operations=0,
                  error_read_operations=0, traced_tasks=0)
    mapped = set()
    per_syscall = {}
    for path in paths:
        counts['traced_tasks'] += 1
        terminated = False
        with Path(path).open() as stream:
            for number, raw in enumerate(stream, 1):
                line = raw.strip()
                if line == '+++ exited with 0 +++':
                    terminated = True
                    continue
                if line.startswith('--- '):
                    continue
                if not line or '<unfinished ...>' in line or 'resumed>' in line:
                    raise ValueError(f'incomplete trace at {path}:{number}')
                call = re.fullmatch(r'(\w+)\((.*)\)\s+=\s+(.+)', line)
                if not call or call[1] not in TRACE_CALLS:
                    raise ValueError(f'unsupported trace at {path}:{number}')
                name, args, result = call.groups()
                succeeded = not result.startswith('-1 ') and result != '-1'
                annotations = re.findall(r'\d+<([^>]*)>', args)
                if any(source in item and item != source for item in annotations):
                    raise ValueError('source descriptor changed identity or could not be attributed')
                if name == 'io_uring_setup' and succeeded:
                    raise ValueError('io_uring may bypass observed source read syscalls')
                if name in TRANSFERS and source in annotations and succeeded:
                    raise ValueError(f'unsupported source transfer: {name}')
                if name == 'mmap' and succeeded:
                    if source in annotations:
                        raise ValueError('source mmap reads cannot be counted as read syscalls')
                    mapped.update(p for p in annotations if p.startswith('/'))
                if name not in READS:
                    continue
                fd = re.match(r'\d+<([^>]*)>,', args)
                if not fd:
                    # Even a failing unattributed descriptor can hide a missing
                    # source path; do not silently turn absent evidence into zero.
                    raise ValueError(f'unattributed read descriptor at {path}:{number}')
                returned = re.match(r'(-?\d+)(?:\s|$)', result)
                if not returned:
                    raise ValueError(f'unsupported read return at {path}:{number}')
                if fd[1] != source:
                    continue
                value = int(returned[1])
                if value < 0:
                    counts['error_read_operations'] += 1
                elif value == 0:
                    counts['zero_read_operations'] += 1
                else:
                    counts['read_operations'] += 1
                    counts['read_bytes'] += value
                    entry = per_syscall.setdefault(name, dict(read_bytes=0, read_operations=0))
                    entry['read_bytes'] += value
                    entry['read_operations'] += 1
        if not terminated:
            raise ValueError(f'trace task did not exit successfully: {path}')
    if not counts['traced_tasks'] or not counts['read_operations']:
        raise ValueError('no attributable source reads observed')
    return dict(counts, per_syscall=per_syscall, mapped_files=sorted(mapped),
                boundary='successful source-file read/pread/readv syscalls; includes tool hash scan; '
                         'no hash subtraction; not physical-device or NFS traffic')


def validate_result(result, output, args, source_identity):
    manifest, metrics = result
    if json.loads((output / 'manifest.json').read_text()) != manifest:
        raise ValueError('stdout manifest differs from immutable manifest')
    if json.loads((output / 'preparation.json').read_text()) != metrics:
        raise ValueError('stdout metrics differ from stored preparation metrics')
    identity, profile = manifest['identity'], manifest['identity']['profile']
    expected = dict(width=args.width, height=args.height, bits_per_sample=args.bits,
                    components=len(args.bands), tile_edge=args.tile,
                    decomposition_levels=args.levels, bits_per_pixel=args.bpp)
    if (profile != expected or identity['bands'] != args.bands or
            identity['codec_revision'] != args.codec_revision or
            identity['source_sha256'] != source_identity['sha256'] or
            manifest['target'] != args.target):
        raise ValueError('prepared source/profile/codec/target identity mismatch')
    if metrics['source_outer_driver'] != 'NITF' or metrics['source_nitf_ic'] != 'C8':
        raise ValueError('preparation did not use the required NITF C8 route')
    if metrics.get('source_index_required') is not True:
        raise ValueError('preparation did not require a retained source index')
    if metrics['source_hash_read_bytes'] != source_identity['bytes']:
        raise ValueError('tool hash scan byte count differs from source length')
    if metrics['peak_rss_kib'] is None:
        raise ValueError('Linux peak RSS observation unavailable')
    payload = file_identity(output / 'payload.j2c')
    if (payload['sha256'] != identity['payload_sha256'] or
            payload['bytes'] != manifest['encoded_bytes']):
        raise ValueError('immutable payload digest/length mismatch')
    descriptors = []
    for number, expected_hash in enumerate(manifest['descriptor_sha256']):
        item = file_identity(output / 'descriptors' / f'{number}.bin')
        if item['sha256'] != expected_hash:
            raise ValueError('immutable descriptor digest mismatch')
        descriptors.append(item)
    return dict(tid=manifest['tid'], manifest=file_identity(output / 'manifest.json'),
                payload=payload, descriptors=descriptors, profile=profile,
                metrics=metrics, encoded_bytes=manifest['encoded_bytes'])


def evaluate_bounds(limits, report):
    representation = report['representation']
    if (limits['schema'] != 'viewer-nitf-preparation-thresholds/1' or
            limits['frozen_unix_ms'] >= report['started_unix_ms'] or
            limits['source_sha256'] != report['source']['sha256'] or
            limits['profile'] != representation['profile'] or
            limits['bands'] != report['bands'] or
            limits['measurement_mode'] != report['measurement_mode'] or
            limits['source_cache_state'] != report['source_cache_state']):
        raise ValueError('NITF freeze identity/profile/mode/cache/time mismatch')
    observed = dict(representation['metrics'], wall_ms=report['wall_ms'],
                    encoded_bytes=representation['encoded_bytes'],
                    descriptor_to_encoded_ratio=representation['metrics']['descriptor_bytes'] /
                    representation['encoded_bytes'])
    if report.get('source_syscalls'):
        observed.update(source_read_bytes=report['source_syscalls']['read_bytes'],
                        source_read_operations=report['source_syscalls']['read_operations'])
    failures = []
    if not limits['bounds']:
        raise ValueError('qualification requires non-empty bounds')
    for name, rule in limits['bounds'].items():
        if set(rule) != {'maximum'} and set(rule) != {'exact'}:
            raise ValueError(f'unsupported bound: {name}')
        comparison, bound = next(iter(rule.items()))
        value = observed.get(name)
        if (type(value) not in (int, float) or type(bound) not in (int, float) or
                not math.isfinite(value) or not math.isfinite(bound) or bound < 0):
            raise ValueError(f'unavailable/invalid numeric bound: {name}')
        violated = value > bound if comparison == 'maximum' else value != bound
        if violated:
            failures.append(f'{name}: {value} violates {comparison} {bound}')
    return failures


def revision(value):
    if not re.fullmatch('[0-9a-f]{40}', value):
        raise argparse.ArgumentTypeError('a full lowercase Git revision is required')
    return value


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('tool', 'gdal-library', 'plugin', 'codec-library', 'input', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ('tool', 'gdal', 'plugin', 'codec'):
        parser.add_argument('--' + name + '-revision', type=revision, required=True)
    parser.add_argument('--build-record', type=Path, action='append', required=True,
                        help='repeat for retained build logs/manifests tying binary hashes to revisions')
    parser.add_argument('--measurement-mode', choices=('observer', 'performance'), required=True)
    parser.add_argument('--source-cache-state', choices=('cold-os', 'warm-os', 'uncontrolled'),
                        default='uncontrolled')
    parser.add_argument('--target', required=True)
    parser.add_argument('--width', type=int, required=True)
    parser.add_argument('--height', type=int, required=True)
    parser.add_argument('--bits', type=int, choices=range(8, 17), required=True)
    parser.add_argument('--bands', required=True)
    parser.add_argument('--tile', type=int, choices=(256, 512, 1024), required=True)
    parser.add_argument('--levels', type=int, choices=(2, 5, 6), required=True)
    parser.add_argument('--bpp', type=float, required=True)
    parser.add_argument('--timeout-seconds', type=float, default=3600)
    parser.add_argument('--limits', type=Path)
    args = parser.parse_args(argv)
    args.bands = [int(band) for band in args.bands.split(',')]
    if (len(args.bands) not in (1, 3) or len(set(args.bands)) != len(args.bands) or
            min(args.bands) < 1 or min(args.width, args.height) < 1 or
            not math.isfinite(args.bpp) or args.bpp <= 0 or
            not math.isfinite(args.timeout_seconds) or args.timeout_seconds <= 0):
        parser.error('invalid bands/geometry/rate/timeout')
    if platform.system() != 'Linux' or platform.machine() not in ('x86_64', 'aarch64'):
        parser.error('observer syscall coverage is supported on Linux x86_64/aarch64 only')
    # Reject reuse before hashing or invoking anything; failed evidence remains
    # available for inspection and is never regenerated inside the same root.
    args.output.mkdir(parents=True, exist_ok=False)
    root = args.output.resolve()
    report = dict(schema='viewer-nitf-preparation-result/1', completed=False,
                  qualified=False, phase='qualification' if args.limits else 'baseline',
                  started_unix_ms=int(time.time() * 1000), failures=[],
                  measurement_mode=args.measurement_mode, bands=args.bands,
                  timing_boundary='subprocess wall time; strace overhead included' if
                  args.measurement_mode == 'observer' else 'uninstrumented subprocess wall time',
                  cache_state='uncontrolled; wrapper identity hash pass precedes timed preparation'
                  if args.source_cache_state == 'uncontrolled' else
                  f'{args.source_cache_state}; source-file residency observed before launch',
                  source_cache_state=args.source_cache_state,
                  source_cache=dict(boundary='source-file OS page cache residency snapshots; '
                                    'not device or NFS cache state; concurrent access/eviction '
                                    'can change residency after observation'),
                  invocation=sys.argv if argv is None else argv,
                  wrapper=file_identity(Path(__file__)),
                  environment={key: os.environ[key] for key in ENV_KEYS if key in os.environ},
                  host=dict(system=platform.system(), release=platform.release(),
                            machine=platform.machine(), python=platform.python_version()))
    try:
        paths = {name: safe_trace_path(getattr(args, name)) for name in
                 ('tool', 'gdal_library', 'plugin', 'codec_library', 'input')}
        report['source'] = file_identity(paths['input'])
        report['binaries'] = {name: file_identity(paths[name]) for name in
                              ('tool', 'gdal_library', 'plugin', 'codec_library')}
        report['declared_build_revisions'] = {name: getattr(args, name + '_revision') for name in
                                              ('tool', 'gdal', 'plugin', 'codec')}
        report['build_records'] = [file_identity(p) for p in args.build_record]
        environment = report['environment']
        environment.update(GDAL_DRIVER_PATH=str(Path(paths['plugin']).parent),
                           JP2EMUELLA_REQUIRE_SOURCE_INDEX='YES',
                           GDAL_PAM_ENABLED='NO', GDAL_NUM_THREADS='1',
                           OMP_NUM_THREADS='1', OPENBLAS_NUM_THREADS='1')
        limits = json.loads(args.limits.read_text()) if args.limits else None
        if args.limits:
            report['thresholds'] = file_identity(args.limits)
        prepared = root / 'representation'
        command = [paths['tool'], 'prepare', '--gdal-library', paths['gdal_library'],
                   '--input', paths['input'], '--output', str(prepared), '--target', args.target,
                   '--bits', str(args.bits), '--bands', ','.join(map(str, args.bands)),
                   '--tile', str(args.tile), '--levels', str(args.levels), '--bpp', str(args.bpp),
                   '--codec-revision', args.codec_revision]
        report['prepare_command'] = command
        if args.measurement_mode == 'observer':
            strace = shutil.which('strace')
            if not strace:
                raise ValueError('strace required for observer mode')
            report['observer'] = dict(binary=file_identity(strace), version=subprocess.check_output(
                [strace, '--version'], text=True, env=environment).splitlines()[0])
            command = trace_command(strace, root / 'source.strace', command)
        report['executed_command'] = command
        with (root / 'result.json').open('wb') as stdout, (root / 'stderr.txt').open('wb') as stderr:
            condition_source_cache(paths['input'], args.source_cache_state, report['source_cache'])
            start = time.monotonic()
            report['source_cache']['launch_started_monotonic_ns'] = time.monotonic_ns()
            process = subprocess.Popen(command, stdout=stdout, stderr=stderr, env=environment,
                                       start_new_session=True)
            try:
                report['returncode'] = process.wait(timeout=args.timeout_seconds)
            except (subprocess.TimeoutExpired, KeyboardInterrupt):
                # strace and its tracees share this new process group. Kill the
                # complete invocation so a timeout cannot leave preparation running.
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
                raise
            finally:
                report['wall_ms'] = (time.monotonic() - start) * 1000
                observe_source_cache(paths['input'], 'after', args.source_cache_state,
                                     report['source_cache'])
        cache = report['source_cache']
        if (cache['before']['available'] and cache['after']['available'] and
                cache['before']['file'] != cache['after']['file']):
            raise ValueError('source file identity changed across preparation')
        if report['returncode']:
            raise ValueError(f'preparation failed with exit {report["returncode"]}; see stderr.txt')
        report['result'] = file_identity(root / 'result.json')
        report['representation'] = validate_result(json.loads((root / 'result.json').read_text()),
                                                   prepared, args, report['source'])
        if args.measurement_mode == 'observer':
            traces = sorted(root.glob('source.strace.*'))
            report['trace_files'] = [file_identity(p) for p in traces]
            report['source_syscalls'] = parse_source_trace(traces, paths['input'])
            mapped = report['source_syscalls']['mapped_files']
            if any(paths[name] not in mapped for name in ('gdal_library', 'plugin', 'codec_library')):
                raise ValueError('explicit GDAL library/plugin/codec C API mappings not observed')
            if report['source_syscalls']['read_bytes'] < report['source']['bytes']:
                raise ValueError('observed source bytes do not cover even the deliberate hash scan')
        else:
            report['source_syscalls'] = None
        if file_identity(paths['input']) != report['source']:
            raise ValueError('source changed during preparation')
        for name, identity in report['binaries'].items():
            if file_identity(paths[name]) != identity:
                raise ValueError('binary changed during preparation')
        if limits:
            report['failures'].extend(evaluate_bounds(limits, report))
        report['completed'] = not report['failures']
        report['qualified'] = bool(limits) and report['completed']
    except (OSError, ValueError, KeyError, TypeError, AttributeError, subprocess.SubprocessError) as error:
        report['failures'].append(str(error))
    for name, path in (('result', root / 'result.json'), ('stderr', root / 'stderr.txt')):
        if path.is_file():
            report[name] = file_identity(path)
    if args.measurement_mode == 'observer':
        report['trace_files'] = [file_identity(p) for p in sorted(root.glob('source.strace.*'))]
    (root / 'preparation-result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({key: report[key] for key in ('completed', 'qualified', 'phase', 'failures')}))
    return 0 if report['completed'] else 4


if __name__ == '__main__':
    raise SystemExit(main())
