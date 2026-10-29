import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

SUPERVISOR=Path(__file__).with_name('supervise.py')
class SupervisorTests(unittest.TestCase):
    def test_reaps_child_spawned_just_before_parent_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            pid_file=Path(directory)/'pid'
            code="import subprocess,sys;from pathlib import Path;p=subprocess.Popen([sys.executable,'-c','import time;time.sleep(60)']);Path(sys.argv[1]).write_text(str(p.pid))"
            result=subprocess.run([sys.executable,str(SUPERVISOR),sys.executable,'-c',code,str(pid_file)],capture_output=True,text=True,timeout=10)
            self.assertTrue(result.stdout,result.stderr)
            report=json.loads(result.stdout)
            self.assertTrue(report['forcedCleanup'])
            self.assertEqual(report['remainingOwnedChildren'],0)
            self.assertGreaterEqual(report['reapedOrphans'],1)
            self.assertFalse(Path('/proc',pid_file.read_text()).exists())

    def test_unrelated_process_survives_orphan_cleanup(self):
        unrelated=subprocess.Popen([sys.executable,'-c','import time;time.sleep(60)'])
        try:
            code="import subprocess,sys;subprocess.Popen([sys.executable,'-c','import time;time.sleep(60)'])"
            result=subprocess.run([sys.executable,str(SUPERVISOR),sys.executable,'-c',code],capture_output=True,text=True,timeout=10)
            report=json.loads(result.stdout)
            self.assertEqual(report['remainingOwnedChildren'],0)
            self.assertIsNone(unrelated.poll())
        finally:
            unrelated.terminate();unrelated.wait()

    def test_sigterm_during_active_parent_reaps_all_descendants(self):
        with tempfile.TemporaryDirectory() as directory:
            pid_file=Path(directory)/'pid'
            code="import os,subprocess,sys,time;from pathlib import Path;p=subprocess.Popen([sys.executable,'-c','import time;time.sleep(60)']);Path(sys.argv[1]).write_text(str(p.pid));time.sleep(60)"
            child=subprocess.Popen([sys.executable,str(SUPERVISOR),sys.executable,'-c',code,str(pid_file)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            try:
                deadline=time.monotonic()+5
                while not pid_file.exists() and time.monotonic()<deadline:time.sleep(.01)
                self.assertTrue(pid_file.exists())
                child.send_signal(signal.SIGTERM)
                out,err=child.communicate(timeout=10)
                report=json.loads(out)
                self.assertTrue(report['forcedCleanup'])
                self.assertEqual(report['remainingOwnedChildren'],0)
                self.assertFalse(Path('/proc',pid_file.read_text()).exists(),err)
            finally:
                if child.poll() is None:child.kill();child.wait()

if __name__=='__main__':unittest.main()
