#!/usr/bin/env python3
"""Bounded Linux process diagnostics, separate from viewer memory acceptance."""
import errno
import heapq
import json
from pathlib import Path
import re
import time


INTERVAL_NS = 1_000_000_000
MAX_ROUNDS = 661
MAX_PIDS_PER_ROUND = 32
MAX_IDENTITIES = 256
MAX_ROWS = 8192
MAX_MARKERS = 256
MAX_MARKER_BYTES = 256 * 1024
MAX_STAT_BYTES = 16 * 1024
MAX_STATUS_BYTES = 64 * 1024
MAX_ROLLUP_BYTES = 64 * 1024
MAX_SMAPS_BYTES = 2 * 1024 * 1024
MAX_MAPPINGS = 4096
MAX_PHASE_GAP_NS = 1_500_000_000
FIELDS = ('Rss', 'Pss', 'Private_Clean', 'Private_Dirty', 'Shared_Clean',
          'Shared_Dirty', 'Anonymous', 'Swap', 'SwapPss')
CATEGORIES = ('heap', 'stack', 'anonymous', 'shared_library', 'file', 'device', 'special')
HEADER = re.compile(r'^([0-9a-fA-F]+)-([0-9a-fA-F]+)\s+([rwxps-]{4})\s+'
                    r'[0-9a-fA-F]+\s+[0-9a-fA-F]+:[0-9a-fA-F]+\s+\d+(?:\s+(.*))?$')


def unavailable(reason):
    return {'value': None, 'unavailable_reason': reason}


def available(value):
    return {'value': value, 'unavailable_reason': None}


def read_bounded(path, limit):
    """Never retain raw proc text or include paths/OS exception strings in output."""
    try:
        with path.open('rb') as stream:
            data = stream.read(limit + 1)
    except OSError as error:
        reason = {errno.ENOENT: 'file absent or process exited',
                  errno.ESRCH: 'process exited', errno.EACCES: 'permission denied',
                  errno.EPERM: 'permission denied'}.get(error.errno, 'proc read failed')
        return None, reason
    if len(data) > limit:
        return None, 'byte limit exceeded; partial observation discarded'
    try:
        return data.decode('utf-8'), None
    except UnicodeDecodeError:
        return None, 'invalid UTF-8; observation discarded'


def parse_stat(text, expected_pid):
    # comm can contain spaces and closing parentheses. Fields after its final
    # closing parenthesis start at field 3; starttime is field 22.
    opening, closing = text.find('('), text.rfind(')')
    if opening < 1 or closing <= opening or int(text[:opening].strip()) != expected_pid:
        raise ValueError('malformed process identity')
    fields = text[closing + 1:].split()
    if len(fields) < 20 or len(fields[0]) != 1:
        raise ValueError('truncated process identity')
    parent, start = int(fields[1]), int(fields[19])
    if parent < 0 or start < 0:
        raise ValueError('negative process identity')
    return {'pid': expected_pid, 'ppid': parent, 'start_time_ticks': start}


def parse_kib_fields(text, names):
    values = {name: unavailable('field absent') for name in names}
    seen = set()
    for line in text.splitlines():
        name, separator, raw = line.partition(':')
        if not separator or name not in values:
            continue
        if name in seen:
            raise ValueError('duplicate memory field')
        seen.add(name)
        parts = raw.split()
        if len(parts) != 2 or parts[1] != 'kB' or not parts[0].isascii() or not parts[0].isdigit():
            raise ValueError('invalid memory field or unit')
        values[name] = available(int(parts[0]) * 1024)
    return values


def mapping_category(path):
    if path == '[heap]':
        return 'heap'
    if path == '[stack]' or path.startswith('[stack:'):
        return 'stack'
    if not path or path.startswith('[anon:') or path.startswith('[anon_shmem:'):
        return 'anonymous'
    if path.startswith('/dev/') or path.startswith('/memfd:'):
        return 'device'
    if path.startswith('['):
        return 'special'
    if re.search(r'\.so(?:\.|$| )', path):
        return 'shared_library'
    return 'file'


def parse_smaps(text, max_mappings=MAX_MAPPINGS):
    """Summarise complete VMAs only; addresses and filenames never leave here."""
    totals = {category: {'mapping_count': 0, 'virtual_bytes': 0,
                         'fields_bytes': {name: available(0) for name in FIELDS}}
              for category in CATEGORIES}
    current = None
    field_lines = []
    count = 0

    def finish():
        if current is None:
            return
        fields = parse_kib_fields('\n'.join(field_lines), FIELDS)
        target = totals[current]['fields_bytes']
        for name, observation in fields.items():
            if observation['value'] is None:
                target[name] = unavailable('field absent in at least one mapping')
            elif target[name]['value'] is not None:
                target[name]['value'] += observation['value']

    for line in text.splitlines():
        header = HEADER.match(line)
        if header:
            finish()
            count += 1
            if count > max_mappings:
                raise ValueError('mapping count limit exceeded; partial observation discarded')
            start, end = int(header[1], 16), int(header[2], 16)
            if end <= start:
                raise ValueError('invalid mapping range')
            current = mapping_category(header[4] or '')
            totals[current]['mapping_count'] += 1
            totals[current]['virtual_bytes'] += end - start
            field_lines = []
        elif current is None or (line and not re.match(r'^[A-Za-z_]+:', line)):
            raise ValueError('malformed mapping record')
        else:
            # Retain only the fixed memory fields, never arbitrary smaps text.
            if line.partition(':')[0] in FIELDS:
                field_lines.append(line)
    finish()
    if not count:
        raise ValueError('no mappings exposed')
    return totals


def read_identity(directory, pid):
    raw, reason = read_bounded(directory / 'stat', MAX_STAT_BYTES)
    if reason:
        return None, reason
    try:
        return parse_stat(raw, pid), None
    except (ValueError, IndexError):
        return None, 'malformed or truncated process identity'


def proc_observation(directory, filename, limit, parser):
    raw, reason = read_bounded(directory / filename, limit)
    if reason:
        return unavailable(reason)
    try:
        return available(parser(raw))
    except ValueError as error:
        return unavailable(str(error))


def read_process(proc_root, pid, deep=False, clock=time.monotonic_ns):
    directory = proc_root / str(pid)
    begin = clock()
    identity, reason = read_identity(directory, pid)
    row = {'pid': pid, 'identity': unavailable(reason) if reason else available(identity),
           'read_started_monotonic_ns': begin}
    if reason:
        for field in ('status', 'smaps_rollup', 'mapping_categories'):
            row[field] = unavailable('process identity unavailable: ' + reason)
    else:
        row['status'] = proc_observation(directory, 'status', MAX_STATUS_BYTES,
                                         lambda raw: parse_kib_fields(raw, ('VmRSS', 'VmHWM')))
        row['smaps_rollup'] = (proc_observation(directory, 'smaps_rollup', MAX_ROLLUP_BYTES,
                                              lambda raw: parse_kib_fields(raw, FIELDS))
                               if deep else unavailable('not selected for this bounded deep sample'))
        row['mapping_categories'] = (proc_observation(directory, 'smaps', MAX_SMAPS_BYTES, parse_smaps)
                                      if deep else unavailable('not selected for this bounded deep sample'))
        after, after_reason = read_identity(directory, pid)
        if after_reason or after['start_time_ticks'] != identity['start_time_ticks']:
            reason = ('process identity unavailable after read: ' + after_reason if after_reason
                      else 'PID start time changed during read; mixed observation discarded')
            for field in ('identity', 'status', 'smaps_rollup', 'mapping_categories'):
                row[field] = unavailable(reason)
    row['read_finished_monotonic_ns'] = clock()
    return row


def read_markers(path):
    raw, reason = read_bounded(path, MAX_MARKER_BYTES)
    if reason:
        return unavailable(reason)
    lines = raw.splitlines()
    if len(lines) > MAX_MARKERS:
        return unavailable('phase marker count limit exceeded; partial observation discarded')
    markers = []
    for line in lines:
        try:
            item = json.loads(line)
            if not isinstance(item, dict):
                raise ValueError()
            # Whitelist scalar metadata; no event payloads or paths are exported.
            if item.get('schema') != 'viewer_memory_phase_marker/1' or item.get('clock') != 'linux_monotonic':
                raise ValueError()
            for key in ('pid', 'start_time_ticks', 'monotonic_ns', 'sequence'):
                if type(item.get(key)) is not int or item[key] < 0:
                    raise ValueError()
            if item['pid'] == 0:
                raise ValueError()
            for key in ('phase_label', 'kind'):
                if not isinstance(item.get(key), str) or not re.fullmatch(r'[A-Za-z0-9_.:-]{1,96}', item[key]):
                    raise ValueError()
            if item['kind'] not in ('startup', 'graphics-ready', 'phase-start', 'phase-settled', 'cycle-evicted', 'cycle-revisited'):
                raise ValueError()
            if markers and (item['sequence'] <= markers[-1]['sequence'] or item['monotonic_ns'] < markers[-1]['monotonic_ns']):
                raise ValueError()
            markers.append({key: item[key] for key in ('schema', 'clock', 'pid', 'start_time_ticks',
                                                       'monotonic_ns', 'sequence', 'phase_label', 'kind')})
        except (ValueError, TypeError, OverflowError, RecursionError):
            return unavailable('invalid phase marker schema, identity, ordering or clock; observation discarded')
    return available(markers) if markers else unavailable('no phase markers emitted')


def attach_markers(markers, samples):
    """Attach bracketing reads, never interpolate RSS or invent phase peaks."""
    attachments = []
    for marker in markers:
        before, after = None, None
        for index, row in enumerate(samples):
            identity = row['identity']['value']
            if identity is None or (identity['pid'], identity['start_time_ticks']) != (marker['pid'], marker['start_time_ticks']):
                continue
            start, finish = row['read_started_monotonic_ns'], row['read_finished_monotonic_ns']
            point = marker['monotonic_ns']
            if finish <= point and point - finish <= MAX_PHASE_GAP_NS:
                if before is None or finish > samples[before]['read_finished_monotonic_ns']:
                    before = index
            if start >= point and start - point <= MAX_PHASE_GAP_NS:
                if after is None or start < samples[after]['read_started_monotonic_ns']:
                    after = index
        attachments.append({'marker': marker,
                            'before_sample': available(before) if before is not None else unavailable('no same-identity read wholly before marker within 1500 ms'),
                            'after_sample': available(after) if after is not None else unavailable('no same-identity read wholly after marker within 1500 ms'),
                            'at_marker_rss_bytes': unavailable('external reads bracket markers; instantaneous RSS is not observed')})
    return attachments


class MemoryDiagnostics:
    def __init__(self, proc_root=Path('/proc'), clock=time.monotonic_ns):
        self.proc_root, self.clock = proc_root, clock
        self.samples, self.identities = [], {}
        self.rounds = self.pid_cursor = self.deep_cursor = 0
        self.next_ns = 0
        self.skipped = {'pid_slots': 0, 'row_slots': 0, 'identity_slots': 0, 'rounds': 0}
        boot, reason = read_bounded(proc_root / 'sys/kernel/random/boot_id', 128)
        if not reason and not re.fullmatch(r'[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}\s*', boot):
            reason = 'invalid boot identity'
        self.boot_id = unavailable(reason) if reason else available(boot.strip())

    def sample(self, pids):
        now = self.clock()
        if now < self.next_ns:
            return
        self.next_ns = now + INTERVAL_NS
        if self.rounds >= MAX_ROUNDS:
            self.skipped['rounds'] += 1
            return
        self.rounds += 1
        if not pids:
            return
        # Keep only a bounded selection, even when the existing process-tree
        # sampler supplies a large descendant set. Wrap by numeric PID.
        selected = heapq.nsmallest(MAX_PIDS_PER_ROUND, (pid for pid in pids if pid > self.pid_cursor))
        remaining = MAX_PIDS_PER_ROUND - len(selected)
        if remaining:
            selected += heapq.nsmallest(remaining, (pid for pid in pids if pid <= self.pid_cursor))
        self.pid_cursor = selected[-1]
        self.skipped['pid_slots'] += len(pids) - len(selected)
        # Rotate deep reads independently so fixed-size PID batches cannot
        # repeatedly select only a subset of the population.
        deep_pid = min((pid for pid in pids if pid > self.deep_cursor), default=None)
        if deep_pid is None:
            deep_pid = min(pids)
        if deep_pid not in selected:
            selected[-1] = deep_pid
        self.deep_cursor = deep_pid
        for pid in selected:
            if len(self.samples) >= MAX_ROWS:
                self.skipped['row_slots'] += 1
                continue
            row = read_process(self.proc_root, pid, deep=pid == deep_pid, clock=self.clock)
            identity = row['identity']['value']
            if identity is not None:
                key = (pid, identity['start_time_ticks'])
                if key not in self.identities and len(self.identities) >= MAX_IDENTITIES:
                    self.skipped['identity_slots'] += 1
                    continue
                entry = self.identities.setdefault(key, {'identity': identity, 'first_sample': len(self.samples),
                                                        'last_sample': len(self.samples),
                                                        'observed_high_water_bytes': unavailable('VmHWM unavailable')})
                entry['last_sample'] = len(self.samples)
                status = row['status']['value']
                hwm = status['VmHWM']['value'] if status else None
                if hwm is not None:
                    previous = entry['observed_high_water_bytes']['value']
                    entry['observed_high_water_bytes'] = available(max(previous or 0, hwm))
            self.samples.append(row)

    def report(self, marker_path, stages):
        markers = read_markers(marker_path)
        phase_observations = []
        for index, stage in enumerate(stages[:MAX_MARKERS]):
            # The authored workload supplies labels; never copy complete snapshots.
            label = stage.get('phase_label')
            safe_label = label if isinstance(label, str) and re.fullmatch(r'[A-Za-z0-9_.:-]{1,96}', label) else None
            hwm = stage.get('process_peak_rss_bytes')
            phase_observations.append({'stage_index': index, 'phase_label': safe_label,
                                       'snapshot_high_water_bytes': available(hwm) if type(hwm) is int and hwm >= 0 else unavailable('application snapshot did not expose process high water'),
                                       'external_memory': unavailable('application-relative phase clock has no external monotonic anchor; see separately validated phase markers')})
        return {'schema': 'viewer_memory_diagnostics/1', 'boot_id': self.boot_id,
                'clock': 'linux_monotonic',
                'boundary': 'diagnostic proc reads only; does not change process-memory acceptance; shared mappings may repeat across PIDs; fields overlap and must not be added to RSS',
                'limits': {'interval_ms': INTERVAL_NS // 1_000_000, 'rounds': MAX_ROUNDS,
                           'pids_per_round': MAX_PIDS_PER_ROUND, 'identities': MAX_IDENTITIES,
                           'rows': MAX_ROWS, 'deep_reads_per_round': 1,
                           'stat_bytes': MAX_STAT_BYTES, 'status_bytes': MAX_STATUS_BYTES,
                           'rollup_bytes': MAX_ROLLUP_BYTES, 'smaps_bytes': MAX_SMAPS_BYTES,
                           'mappings_per_read': MAX_MAPPINGS, 'markers': MAX_MARKERS,
                           'marker_bytes': MAX_MARKER_BYTES, 'phase_gap_ms': MAX_PHASE_GAP_NS // 1_000_000},
                'rounds_observed': self.rounds, 'skipped': self.skipped,
                'identities': list(self.identities.values()), 'samples': self.samples,
                'phase_markers': markers,
                'phase_attachments': attach_markers(markers['value'] or [], self.samples),
                'application_phase_observations': phase_observations,
                'application_phases_omitted': max(0, len(stages) - MAX_MARKERS),
                'allocation_lifetime_attribution': unavailable('proc exposes residency and mapping categories, not allocation call sites, live allocation totals or retained allocator capacity'),
                'mapping_boundary': 'heuristic pathname categories; smaps resident fields are separate from virtual address ranges; device mappings do not measure physical GPU memory; no filenames or addresses retained',
                'sampling_limit': 'one deep PID read per round; reads are sequential, not atomic; a stable start time does not detect exec; short-lived or reparented processes can be missed; collection adds diagnostic overhead'}
