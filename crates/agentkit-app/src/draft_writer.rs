//! Coalesced, serial draft RPC writes. Queued snapshots survive reconnects and view changes.
use crate::draft_io::Snapshot;
use agentkit_client::api::WorkbenchApi;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const DEBOUNCE: Duration = Duration::from_millis(500);
#[derive(Clone)]
pub(crate) struct Notice {
    pub session: String,
    pub sequence: u64,
    pub result: Result<(), String>,
}
struct Pending {
    sequence: u64,
    draft: Arc<Snapshot>,
    due: Instant,
    failed: bool,
}
#[derive(Default)]
struct Queue {
    api: Option<WorkbenchApi>,
    binding: u64,
    sequence: u64,
    pending: BTreeMap<String, Pending>,
    flushers: Vec<async_channel::Sender<Result<(), String>>>,
}
/// Shared by the controller and orderly-exit path; only this worker writes drafts.
#[derive(Clone)]
pub(crate) struct Writer {
    queue: Arc<Mutex<Queue>>,
    wake: async_channel::Sender<()>,
    pub notices: async_channel::Receiver<Notice>,
    _task: Arc<smol::Task<()>>,
}
impl gpui_kit::Global for Writer {}
fn lock(queue: &Mutex<Queue>) -> std::sync::MutexGuard<'_, Queue> {
    queue
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
impl Writer {
    pub fn new() -> Self {
        let queue = Arc::new(Mutex::new(Queue::default()));
        let (wake, receiver) = async_channel::bounded(1);
        let (notices, out) = async_channel::unbounded();
        let state = queue.clone();
        let task = smol::spawn(async move {
            run(state, receiver, notices).await;
        });
        Self {
            queue,
            wake,
            notices: out,
            _task: Arc::new(task),
        }
    }
    pub fn bind(&self, api: WorkbenchApi) {
        {
            let mut q = lock(&self.queue);
            q.api = Some(api);
            q.binding = q.binding.wrapping_add(1);
            for p in q.pending.values_mut() {
                p.failed = false;
            }
        }
        let _ = self.wake.try_send(());
    }
    pub fn unbind(&self) {
        let mut q = lock(&self.queue);
        q.api = None;
        q.binding = q.binding.wrapping_add(1);
    }
    pub fn save(&self, session: String, draft: Snapshot, immediate: bool) -> u64 {
        let sequence = {
            let mut q = lock(&self.queue);
            q.sequence = q.sequence.wrapping_add(1);
            let sequence = q.sequence;
            q.pending.insert(
                session,
                Pending {
                    sequence,
                    draft: Arc::new(draft),
                    due: Instant::now() + if immediate { Duration::ZERO } else { DEBOUNCE },
                    failed: false,
                },
            );
            sequence
        };
        let _ = self.wake.try_send(());
        sequence
    }
    pub fn pending_sequence(&self, session: &str) -> Option<u64> {
        lock(&self.queue).pending.get(session).map(|p| p.sequence)
    }
    pub fn retry(&self, session: &str) {
        if let Some(p) = lock(&self.queue).pending.get_mut(session) {
            p.failed = false;
            p.due = Instant::now();
        }
        let _ = self.wake.try_send(());
    }
    /// Force all queued snapshots before an explicit restart or ordinary app quit.
    pub async fn flush(&self) -> Result<(), String> {
        let (tx, rx) = async_channel::bounded(1);
        {
            let mut q = lock(&self.queue);
            for p in q.pending.values_mut() {
                p.failed = false;
                p.due = Instant::now();
            }
            q.flushers.push(tx);
        }
        let _ = self.wake.try_send(());
        rx.recv()
            .await
            .unwrap_or_else(|_| Err("ak.draft.worker_stopped".into()))
    }
}
async fn run(
    queue: Arc<Mutex<Queue>>,
    wake: async_channel::Receiver<()>,
    notices: async_channel::Sender<Notice>,
) {
    loop {
        let action = {
            let mut q = lock(&queue);
            let ready = q
                .pending
                .iter()
                .filter(|(_, p)| !p.failed)
                .min_by_key(|(_, p)| p.due)
                .map(|(id, p)| (id.clone(), p.sequence, p.draft.clone(), p.due));
            if ready.is_none() || q.api.is_none() {
                let result = if q.pending.is_empty() {
                    Ok(())
                } else {
                    Err("ak.draft.unsaved".into())
                };
                for waiter in q.flushers.drain(..) {
                    let _ = waiter.try_send(result.clone());
                }
            }
            ready.and_then(|(id, sequence, draft, due)| {
                q.api
                    .clone()
                    .map(|api| (id, sequence, draft, due, api, q.binding))
            })
        };
        let Some((id, sequence, draft, due, api, binding)) = action else {
            if wake.recv().await.is_err() {
                break;
            }
            continue;
        };
        if due > Instant::now() {
            match smol::future::race(
                async {
                    smol::Timer::at(due).await;
                    Ok(false)
                },
                async { wake.recv().await.map(|_| true) },
            )
            .await
            {
                Ok(true) => continue,
                Err(_) => break,
                Ok(false) => {}
            }
        }
        // Re-read after a timer: an edit may have moved the deadline or replaced the snapshot.
        let current = {
            let q = lock(&queue);
            q.binding == binding
                && q.pending
                    .get(&id)
                    .is_some_and(|p| p.sequence == sequence && p.due <= Instant::now())
        };
        if !current {
            continue;
        }
        let result = match draft.to_wire() {
            Ok(draft) => match api.bind_connection() {
                Ok((api, _)) => {
                    if draft.text.is_empty() && draft.attachments.as_ref().is_none_or(Vec::is_empty)
                    {
                        api.delete_draft(&id).await
                    } else {
                        api.set_draft(&id, &draft).await
                    }
                }
                Err(e) => Err(e),
            }
            .map_err(|e| e.to_string()),
            Err(key) => Err(key.into()),
        };
        let notify = {
            let mut q = lock(&queue);
            if q.binding != binding {
                false
            } else {
                if q.pending.get(&id).is_some_and(|p| p.sequence == sequence) {
                    if result.is_ok() {
                        q.pending.remove(&id);
                    } else if let Some(p) = q.pending.get_mut(&id) {
                        p.failed = true;
                    }
                }
                true
            }
        };
        if notify {
            let _ = notices
                .send(Notice {
                    session: id,
                    sequence,
                    result,
                })
                .await;
        }
    }
}

#[cfg(test)]
#[path = "draft_writer_tests.rs"]
mod tests;
