//! Platform process ownership through safe dependency APIs.
#[cfg(windows)]
use command_group::CommandGroup;
#[cfg(unix)]
use process_wrap::std::{ProcessSession, StdChildWrapper, StdCommandWrap};
use std::{
    io,
    process::{Child, Command},
    time::{Duration, Instant},
};

pub(crate) struct Tree {
    #[cfg(unix)]
    child: Box<dyn StdChildWrapper>,
    #[cfg(windows)]
    child: command_group::GroupChild,
}
impl Tree {
    pub fn spawn(command: Command) -> io::Result<Self> {
        #[cfg(unix)]
        let child = StdCommandWrap::from(command).wrap(ProcessSession).spawn()?;
        #[cfg(windows)]
        let child = {
            let mut command = command;
            command.group().kill_on_drop(true).spawn()?
        };
        Ok(Self { child })
    }
    pub fn inner(&mut self) -> &mut Child {
        #[cfg(unix)]
        {
            self.child.inner_mut()
        }
        #[cfg(windows)]
        {
            self.child.inner()
        }
    }
    pub fn kill(&mut self) {
        #[cfg(unix)]
        {
            let _ = self.child.start_kill();
        }
        #[cfg(windows)]
        {
            let _ = self.child.kill();
        }
    }
    pub fn terminate(
        &mut self,
        grace: Duration,
        stop: Option<&std::sync::atomic::AtomicBool>,
    ) -> io::Result<()> {
        #[cfg(unix)]
        {
            let _ = self.child.signal(15);
        }
        #[cfg(windows)]
        self.kill();
        let end = Instant::now() + grace;
        while Instant::now() < end {
            if stop.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Acquire)) {
                break;
            }
            let leader_exited = self.inner().try_wait()?.is_some();
            #[cfg(unix)]
            {
                use nix::{errno::Errno, sys::signal::killpg, unistd::Pid};
                let group_gone = matches!(
                    killpg(Pid::from_raw(self.inner().id() as i32), None),
                    Err(Errno::ESRCH)
                );
                if leader_exited && group_gone {
                    return Ok(());
                }
            }
            #[cfg(windows)]
            if leader_exited {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.kill();
        self.inner().wait()?;
        Ok(())
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        self.kill();
        let _ = self.inner().wait();
    }
}
