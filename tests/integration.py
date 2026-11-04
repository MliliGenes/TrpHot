#!/usr/bin/env python3
"""Exercise the real engine and Make in isolated temporary projects."""
import json, os, pathlib, queue, signal, subprocess, tempfile, threading, time
BINARY = pathlib.Path(os.environ.get('TRPHOT_TEST_BINARY', str(pathlib.Path(__file__).resolve().parents[1] / 'target/debug/trphot')))
def alive(pid):
    try:
        stat=pathlib.Path(f'/proc/{pid}/stat')
        if stat.exists() and stat.read_text().split()[2]=='Z': return False
        os.kill(pid,0);return True
    except ProcessLookupError:return False
with tempfile.TemporaryDirectory(prefix='trphot-test-') as td:
    root=pathlib.Path(td)
    (root/'source.c').write_text('one')
    (root/'Makefile').write_text('all:\n\t@sleep 0.2\n\t@test ! -f fail\n\t@printf "built\\n"\n')
    (root/'.trphot.json').write_text(json.dumps({'interval':50,'debounce':100,'ignores':['*.pid','fail'], 'exec':'sleep 1000 & echo $! > app.pid; wait'}))
    p=subprocess.Popen([str(BINARY),str(root),'--engine'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    events=queue.Queue();history=[]
    def reader():
        for line in p.stdout: events.put(json.loads(line))
    threading.Thread(target=reader,daemon=True).start()
    def wait(pred,timeout=8):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            try:e=events.get(timeout=.1)
            except queue.Empty:continue
            history.append(e)
            if pred(e):return e
        raise AssertionError(f'event timeout: {history[-15:]}')
    def cmd(c,**kw):p.stdin.write(json.dumps({'cmd':c,**kw})+'\n');p.stdin.flush()
    def status(text):return lambda e:e.get('type')=='status' and text in e.get('message','')
    try:
        ready=wait(lambda e:e['type']=='ready');assert not ready['active']
        time.sleep(.2);assert not (root/'app.pid').exists()
        # Invalid rule rejected without replacing the current config.
        cfg=ready['config'];bad=dict(cfg,interval=0)
        cmd('config',config=bad);wait(status('interval must'))
        cmd('arm');wait(status('Application started'))
        end=time.monotonic()+3
        while not (root/'app.pid').exists() and time.monotonic()<end:time.sleep(.02)
        pid=int((root/'app.pid').read_text());assert alive(pid)
        # A failed build leaves the old application alive.
        (root/'fail').touch();(root/'source.c').write_text('two')
        wait(lambda e:e.get('success') is False);assert alive(pid)
        # A successful rebuild restarts the application and kills its descendants.
        (root/'fail').unlink();(root/'source.c').write_text('three')
        wait(status('Application started'));assert not alive(pid)
        # Changes during an in-flight build schedule another build.
        (root/'source.c').write_text('four');wait(lambda e:e.get('building') is True)
        (root/'source.c').write_text('five');wait(lambda e:e.get('success') is True)
        wait(lambda e:e.get('building') is True);wait(lambda e:e.get('success') is True)
        (root/'new.h').write_text('new');wait(lambda e:e.get('kind')=='added' and e.get('path')=='new.h')
        (root/'new.h').unlink();wait(lambda e:e.get('kind')=='deleted' and e.get('path')=='new.h')
        cmd('save');wait(status('Saved .trphot.json'));assert json.loads((root/'.trphot.json').read_text())['interval']==50
        cmd('quit');p.wait(timeout=5);assert p.returncode==0
        pid=int((root/'app.pid').read_text());assert not alive(pid)
    finally:
        if p.poll() is None:p.send_signal(signal.SIGTERM);p.wait(timeout=5)
    # One-shot exit codes and CLI validation.
    run=subprocess.run([str(BINARY),str(root),'--once','--exec','exit 7'],capture_output=True,text=True,timeout=5)
    assert run.returncode==1,run.stdout
    run=subprocess.run([str(BINARY),str(root),'--once','--target=-n'],capture_output=True,text=True)
    assert run.returncode==1
    # Spectator headless exits cleanly on SIGTERM.
    p=subprocess.Popen([str(BINARY),str(root),'--headless'],stdout=subprocess.DEVNULL)
    time.sleep(.15);p.terminate();assert p.wait(timeout=5)==0
print('PASS: spectator, validation, build/restart, failed build retention, queued rebuild, file add/delete, save, process cleanup, once exit status, SIGTERM')
with tempfile.TemporaryDirectory(prefix='trphot-output-') as td:
    root=pathlib.Path(td)
    (root/'Makefile').write_text("all:\n\t@printf '#!/bin/sh\\nsleep 1000\\n' > app\n\t@chmod +x app\n")
    outside=root.parent / (root.name+'-outside')
    outside.mkdir();(outside/'hidden.c').write_text('secret')
    (root/'link').symlink_to(outside,target_is_directory=True)
    try:
        p=subprocess.Popen([str(BINARY),str(root),'--engine','--run','--exec','./app','--interval','50'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        events=queue.Queue()
        threading.Thread(target=lambda:[events.put(json.loads(s)) for s in p.stdout],daemon=True).start()
        found=[];end=time.monotonic()+2
        while time.monotonic()<end:
            try:found.append(events.get(timeout=.1))
            except queue.Empty:pass
        assert sum(e.get('building') is True for e in found)==1,found
        ready=next(e for e in found if e['type']=='ready')
        assert not any('hidden.c' in f['path'] for f in ready['files'])
        p.stdin.close();assert p.wait(timeout=5)==0
    finally:
        if p.poll() is None:p.terminate();p.wait(timeout=5)
        (outside/'hidden.c').unlink();outside.rmdir()
print('PASS: generated executable does not cause rebuild loop; symlink traversal excluded; UI stdin close cleans up')
