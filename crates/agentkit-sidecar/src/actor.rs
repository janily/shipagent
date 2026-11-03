//! One thread serializes every child generation and lifecycle command.
use crate::{
    Control, Reply, Shared, SidecarConfig, SidecarError, SidecarEvent, launch::Process, lock,
};
use agentkit_protocol::Endpoint;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};
struct Budget {
    exits: VecDeque<Instant>,
    max: u32,
}
impl Budget {
    fn record(&mut self, now: Instant) -> bool {
        while self
            .exits
            .front()
            .is_some_and(|exit| now.duration_since(*exit) >= Duration::from_secs(60))
        {
            self.exits.pop_front();
        }
        self.exits.push_back(now);
        self.exits.len() <= self.max as usize
    }
}
enum Phase {
    Launch,
    Waiting,
    Running,
    Backoff(Instant),
    Fatal,
}
fn replies(pending: &mut Vec<Reply<Endpoint>>, result: Result<Endpoint, SidecarError>) {
    for reply in pending.drain(..) {
        let _ = reply.try_send(result.clone());
    }
}
fn failure(
    shared: &Shared,
    ready: &mut Option<Reply<()>>,
    pending: &mut Vec<Reply<Endpoint>>,
    error: SidecarError,
) -> bool {
    *lock(&shared.error) = Some(error.clone());
    replies(pending, Err(error.clone()));
    shared.emit(SidecarEvent::Fatal {
        reason: error.clone(),
    });
    if let Some(ready) = ready.take() {
        let _ = ready.try_send(Err(error));
        true
    } else {
        false
    }
}
pub(crate) fn run(
    config: SidecarConfig,
    commands: Receiver<Control>,
    initial: Reply<()>,
    shared: Arc<Shared>,
) {
    let mut ready = Some(initial);
    let mut pending = vec![];
    let mut process: Option<Process> = None;
    let mut phase = Phase::Launch;
    let mut budget = Budget {
        exits: VecDeque::new(),
        max: config.max_restarts,
    };
    loop {
        match commands.recv_timeout(Duration::from_millis(10)) {
            Ok(Control::Shutdown(grace, reply)) => {
                let result = process.as_mut().map(|p| p.finish(grace)).unwrap_or(Ok(()));
                replies(&mut pending, Err(SidecarError::Stopped));
                if let Some(ready) = ready.take() {
                    let _ = ready.try_send(Err(SidecarError::Stopped));
                }
                if let Some(reply) = reply {
                    let _ = reply.try_send(result);
                }
                break;
            }
            Ok(Control::Restart(reply)) => {
                pending.push(reply);
                *lock(&shared.error) = None;
                if !matches!(phase, Phase::Waiting) {
                    if let Some(mut old) = process.take() {
                        let _ = old.finish_for_restart(&shared.stop);
                    }
                    phase = Phase::Launch;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if matches!(phase, Phase::Backoff(until) if Instant::now() >= until) {
            phase = Phase::Launch;
        }
        if matches!(phase, Phase::Launch) {
            match Process::start(&config, &shared.stop) {
                Ok(child) => {
                    process = Some(child);
                    phase = Phase::Waiting;
                }
                Err(error) => {
                    if failure(&shared, &mut ready, &mut pending, error) {
                        break;
                    }
                    phase = Phase::Fatal;
                }
            }
        }
        let Some(child) = process.as_mut() else {
            continue;
        };
        if matches!(phase, Phase::Waiting) {
            let startup = match child.ready.try_recv() {
                Ok(Ok(url)) => {
                    child.endpoint.url = url;
                    Some(Ok(()))
                }
                Ok(Err(error)) => Some(Err(error)),
                Err(_) => None,
            };
            if let Some(startup) = startup {
                match startup {
                    Ok(()) => {
                        let endpoint = child.endpoint.clone();
                        *lock(&shared.endpoint) = endpoint.clone();
                        replies(&mut pending, Ok(endpoint.clone()));
                        if let Some(initial) = ready.take() {
                            if initial.try_send(Ok(())).is_err() {
                                break;
                            }
                        } else {
                            shared.emit(SidecarEvent::Restarted { endpoint });
                        }
                        phase = Phase::Running;
                    }
                    Err(error) => {
                        let _ = child.finish(Duration::from_millis(100));
                        process = None;
                        if failure(&shared, &mut ready, &mut pending, error) {
                            break;
                        }
                        phase = Phase::Fatal;
                        continue;
                    }
                }
            }
        }
        let status = match child.tree.inner().try_wait() {
            Ok(status) => status,
            Err(error) => {
                let _ = child.finish(Duration::ZERO);
                process = None;
                if failure(&shared, &mut ready, &mut pending, error.into()) {
                    break;
                }
                phase = Phase::Fatal;
                continue;
            }
        };
        if let Some(status) = status {
            let code = status.code();
            let _ = child.finish(Duration::ZERO);
            let startup_error = child.startup_error(code);
            let tail = lock(&child.output.tail).clone();
            process = None;
            if matches!(phase, Phase::Waiting) {
                if failure(&shared, &mut ready, &mut pending, startup_error) {
                    break;
                }
                phase = Phase::Fatal;
            } else {
                shared.emit(SidecarEvent::Exited {
                    code,
                    stderr_tail: tail,
                });
                if budget.record(Instant::now()) {
                    phase = Phase::Backoff(Instant::now() + Duration::from_secs(1));
                } else {
                    let _ = failure(
                        &shared,
                        &mut ready,
                        &mut pending,
                        SidecarError::RestartLimit,
                    );
                    phase = Phase::Fatal;
                }
            }
        } else if matches!(phase, Phase::Waiting) && Instant::now() >= child.deadline {
            let _ = child.finish(Duration::from_millis(100));
            let error = if lock(&child.output.occupied).is_some() {
                child.startup_error(None)
            } else {
                SidecarError::ReadyTimeout
            };
            process = None;
            if failure(&shared, &mut ready, &mut pending, error) {
                break;
            }
            phase = Phase::Fatal;
        }
    }
    if let Some(mut child) = process {
        let _ = child.finish(Duration::from_secs(1));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fourth_crash_is_fatal_but_old_crashes_expire_at_sixty_seconds() {
        let now = Instant::now();
        let mut budget = Budget {
            exits: VecDeque::new(),
            max: 3,
        };
        for offset in [0, 1, 2] {
            assert!(budget.record(now + Duration::from_secs(offset)));
        }
        assert!(!budget.record(now + Duration::from_secs(3)));
        assert!(budget.record(now + Duration::from_secs(63)));
    }
}
