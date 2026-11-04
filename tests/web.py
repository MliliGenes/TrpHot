#!/usr/bin/env python3
"""Exercise the embedded dashboard and its connection to the real engine."""
import json
import pathlib
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

BINARY = pathlib.Path(__file__).resolve().parents[1] / 'target/debug/trphot'
with tempfile.TemporaryDirectory(prefix='trphot-web-') as directory:
    root = pathlib.Path(directory)
    (root / 'Makefile').write_text('all:\n\t@echo dashboard-build\n')
    process = subprocess.Popen([str(BINARY), directory, '--no-open'],
                               stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    try:
        line = process.stderr.readline()
        assert line.startswith('TrpHot dashboard: '), line
        url, token = line.strip().split(' ', 2)[2].split('#')

        def request(path, data=None, authenticated=True):
            headers = {'X-Trphot-Token': token} if authenticated else {}
            payload = json.dumps(data).encode() if data is not None else None
            return urllib.request.urlopen(urllib.request.Request(url + path, payload, headers), timeout=3)

        def state():
            return json.load(request('api/state'))

        def wait(predicate):
            end = time.monotonic() + 5
            while time.monotonic() < end:
                result = state()
                if predicate(result):
                    return result
                time.sleep(.05)
            raise AssertionError(result)

        assert b'Project insights' in request('').read()
        assert b'async function poll' in request('app.js').read()
        for path, data in [('api/state', None), ('api/command', {'cmd': 'arm'})]:
            try:
                request(path, data, authenticated=False)
                raise AssertionError('Unauthorized request accepted')
            except urllib.error.HTTPError as error:
                assert error.code == 403
        initial = wait(lambda result: 'config' in result['state'])
        assert initial['state']['active'] is False
        request('api/command', {'cmd': 'build'}).close()
        wait(lambda result: result['state'].get('success') is True)
        wait(lambda result: any(e.get('text') == 'dashboard-build' for e in result['events']))
        (root / 'new.txt').write_text('hello')
        wait(lambda result: any(e.get('path') == 'new.txt' for e in result['events']))
        config = initial['state']['config']
        request('api/command', {'cmd': 'config', 'config': dict(config, interval=0)}).close()
        wait(lambda result: 'interval must' in result['state'].get('message', ''))
        assert state()['state']['config']['interval'] == config['interval']
        request('api/command', {'cmd': 'config', 'config': dict(config, interval=100)}).close()
        wait(lambda result: result['state']['config']['interval'] == 100)
        request('api/command', {'cmd': 'save'}).close()
        wait(lambda result: result['state'].get('message') == 'Saved .trphot.json')
        assert json.loads((root / '.trphot.json').read_text())['interval'] == 100
        # A fresh browser request keeps the same engine and retained activity.
        assert state()['events']
    finally:
        process.terminate()
        assert process.wait(timeout=5) == 0
print('PASS: dashboard assets, authentication, spectator, build, file changes, rules, save, reconnect, shutdown')
