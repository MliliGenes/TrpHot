#!/usr/bin/env python3
import fcntl, os, pathlib, pty, select, struct, subprocess, termios, time
base=pathlib.Path(__file__).resolve().parents[1]
master,slave=pty.openpty()
fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',32,110,0,0))
p=subprocess.Popen([os.environ.get('TRPHOT_TEST_BINARY',str(base/'target/debug/trphot')),str(base/'examples/hello')],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,'TERM':'xterm-256color'},start_new_session=True)
os.close(slave);output=b''
end=time.monotonic()+8
while time.monotonic()<end:
    if select.select([master],[],[],.1)[0]:
        try:output+=os.read(master,65536)
        except OSError:break
    if b'SPECTATOR' in output:break
assert b'SPECTATOR' in output,output[-3000:]
os.write(master,b'q')
end=time.monotonic()+5
while p.poll() is None and time.monotonic()<end:
    if select.select([master],[],[],.1)[0]:
        try:output+=os.read(master,65536)
        except OSError:break
assert p.wait(timeout=3)==0,output[-3000:]
os.close(master)
print('PASS: trphot directory opens real OpenTUI spectator in PTY; q shuts down engine and UI')
