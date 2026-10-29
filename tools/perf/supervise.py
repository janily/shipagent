"""Linux-only child ownership boundary for one native benchmark invocation.

The kernel reparents orphan descendants to this subreaper, so cleanup does not
rely on periodic ancestry snapshots or stale/reused process identifiers.
"""
import ctypes
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


def main(command):
    if sys.platform!='linux' or not command:raise RuntimeError('Linux command required')
    stopping=False
    def stop(_signal,_frame):
        nonlocal stopping
        stopping=True
    signal.signal(signal.SIGTERM,stop)
    signal.signal(signal.SIGINT,stop)
    parent=os.getppid()
    libc=ctypes.CDLL(None,use_errno=True)
    libc.prctl.argtypes=[ctypes.c_int,ctypes.c_ulong,ctypes.c_ulong,ctypes.c_ulong,ctypes.c_ulong]
    for option,value in [(36,1),(1,signal.SIGTERM)]:  # CHILD_SUBREAPER, PDEATHSIG
        if libc.prctl(option,value,0,0,0)!=0:raise OSError(ctypes.get_errno(),'prctl failed')
    if os.getppid()!=parent:stopping=True
    children_path=Path('/proc',str(os.getpid()),'task',str(os.getpid()),'children')
    def children():
        if children_path.exists():return [int(pid) for pid in children_path.read_text().split()]
        # Some kernels omit CONFIG_CHECKPOINT_RESTORE's children file. The
        # subreaper's PID stays alive and owns every matching direct child
        # until waitpid reaps it, so this fallback has no reusable-root race.
        result=[]
        for entry in Path('/proc').iterdir():
            if not entry.name.isdigit():continue
            try:
                raw=(entry/'stat').read_text()
                fields=raw[raw.rfind(')')+2:].split()
                if int(fields[1])==os.getpid():result.append(int(entry.name))
            except (FileNotFoundError,ProcessLookupError,PermissionError):pass
        return result
    app=None;forced=False;reaped=0;error=None;focused=False
    try:
        if stopping:raise RuntimeError('parent exited')
        app=subprocess.Popen(command,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
        deadline=time.monotonic()+65
        focus_deadline=time.monotonic()+5
        crash_ms=int(os.environ.get('AGENTKIT_PERF_CRASH_AFTER_MS','0'))
        crash_at=time.monotonic()+crash_ms/1000 if crash_ms>0 else None
        while app.poll() is None and not stopping and time.monotonic()<deadline:
            if crash_at is not None and time.monotonic()>=crash_at:
                forced=True;app.kill();app.wait(timeout=2);break
            if os.environ.get('AGENTKIT_PERF_FOCUS_X11')=='1' and not focused and time.monotonic()<focus_deadline:
                try:
                    found=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(app.pid)],capture_output=True,text=True,timeout=.5)
                    if found.returncode==0:
                        window=found.stdout.splitlines()[0]
                        focused=subprocess.run(['xdotool','windowfocus','--sync',window],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=.5).returncode==0
                except (FileNotFoundError,subprocess.TimeoutExpired):pass
            time.sleep(.02)
        if app.poll() is None:
            forced=True
            app.terminate()
            try:app.wait(timeout=1)
            except subprocess.TimeoutExpired:
                app.kill()
                try:app.wait(timeout=2)
                except subprocess.TimeoutExpired:error='native-unresponsive'
    except Exception:
        error='launch-or-supervision-failed'
    finally:
        deadline=time.monotonic()+4;terminated=set()
        while children() and time.monotonic()<deadline:
            forced=True
            for pid in children():
                # A kernel-owned unreaped child cannot have its PID reused.
                # We have no SIGCHLD reaper thread and don't reap until below.
                try:
                    if deadline-time.monotonic()<1:os.kill(pid,signal.SIGKILL)
                    elif pid not in terminated:os.kill(pid,signal.SIGTERM);terminated.add(pid)
                except ProcessLookupError:pass
            while True:
                try:pid,_status=os.waitpid(-1,os.WNOHANG)
                except ChildProcessError:break
                if pid==0:break
                reaped+=1
            time.sleep(.02)
        remaining=len(children())
        report={'schemaVersion':1,'nativeExitCode':app.returncode if app else None,'forcedCleanup':forced,'remainingOwnedChildren':remaining,'reapedOrphans':reaped,'error':error,'focusSucceeded':focused}
        print(json.dumps(report),flush=True)
    return 0 if app is not None and app.returncode==0 and not forced and remaining==0 and error is None else 2

if __name__=='__main__':sys.exit(main(sys.argv[1:]))
