//! Bounded subscription delivery and recovery over committed LogRuntime records.
use super::*;
use tokio::sync::mpsc;
use yss_logging::{LogBatchDto, LogRuntime, LogStreamFailure};

pub(super) struct LogLease {
    runtime: LogRuntime,
    id: String,
    executor: tokio::runtime::Handle,
}
impl Drop for LogLease {
    fn drop(&mut self) {
        let runtime = self.runtime.clone();
        let id = self.id.clone();
        self.executor.spawn_blocking(move || {
            if let Err(error) = runtime.unsubscribe(id) {
                tracing::warn!(%error, "Log subscription could not be released");
            }
        });
    }
}

impl LogsPanel {
    pub(super) fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.task = None;
        self.lease = None;
        self.epoch += 1;
        self.connecting = false;
        let Some(runtime) = self.services.logging.logs().cloned() else {
            cx.notify();
            return;
        };
        let epoch = self.epoch;
        let executor = self.services.executor.clone();
        let (sender, mut receiver) = mpsc::channel(64);
        self.connecting = true;
        let task = self.services.executor.spawn_blocking(move || {
            let snapshot =
                runtime.subscribe_batches(move |batch| sender.try_send(batch).is_ok())?;
            let lease = LogLease {
                runtime,
                id: snapshot.subscription_id.clone(),
                executor,
            };
            Ok::<_, yss_logging::LogsUnavailable>((snapshot, lease))
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result.map_err(anyhow::Error::from));
            let (snapshot, lease) = match result {
                Ok(connected) => connected,
                Err(error) => {
                    if let Err(error) = view.update(cx, |view, cx| {
                        if view.epoch == epoch {
                            tracing::warn!(%error, "Log subscription could not be established");
                            view.connecting = false;
                            cx.notify();
                        }
                    }) {
                        tracing::debug!(%error, "Log panel closed before connection failure delivery");
                    }
                    return;
                }
            };
            let accepted = view
                .update(cx, |view, cx| {
                    if view.epoch != epoch {
                        return false;
                    }
                    view.install_snapshot(snapshot, lease, cx);
                    true
                })
                .unwrap_or(false);
            if !accepted {
                return;
            }
            while let Some(batch) = receiver.recv().await {
                let keep = view
                    .update_in(cx, |view, window, cx| {
                        if view.epoch != epoch {
                            return false;
                        }
                        view.accept_batch(batch, window, cx)
                    })
                    .unwrap_or(false);
                if !keep {
                    return;
                }
            }
            if let Err(error) = view.update_in(cx, |view, window, cx| {
                if view.epoch == epoch {
                    // A full host queue closes this sink; recover without an unbounded retry loop.
                    view.recover(window, cx);
                }
            }) {
                tracing::debug!(%error, "Log panel closed before stream recovery");
            }
        }));
        cx.notify();
    }

    fn accept_batch(
        &mut self,
        batch: LogBatchDto,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(failure) = batch.failure {
            self.lease = None;
            if failure == LogStreamFailure::SubscriberLagged {
                self.recover(window, cx);
            }
            return false;
        }
        if batch.stream_id != self.stream
            || batch
                .entries
                .first()
                .is_some_and(|entry| entry.sequence > self.sequence.saturating_add(1))
        {
            self.recover(window, cx);
            return false;
        }
        if !batch.evicted_sequences.is_empty() {
            self.evict_records(&batch.evicted_sequences, cx);
        }
        let following = self.following();
        let previous = self.sequence;
        for record in batch.entries {
            let entry = Rc::new(LogEntry::new(record));
            if entry.record.sequence <= self.sequence {
                continue;
            }
            self.sequence = entry.record.sequence;
            if self.matches(&entry) {
                self.visible.push(self.entries.len());
            }
            self.entries.push_back(entry);
        }
        if self.sequence != previous {
            self.recovery_attempts = 0;
            self.trim(following);
            if following {
                self.scroll.scroll_to_bottom();
            }
            cx.notify();
        }
        true
    }

    fn evict_records(&mut self, sequences: &[u64], cx: &mut Context<Self>) {
        let following = self.following();
        let offset = self.scroll.0.borrow().base_handle.offset();
        let top = (-f32::from(offset.y) / ROW_HEIGHT).floor().max(0.) as usize;
        let removed_above = self
            .visible
            .iter()
            .take(top)
            .filter(|index| {
                sequences
                    .binary_search(&self.entries[**index].record.sequence)
                    .is_ok()
            })
            .count();
        self.entries
            .retain(|entry| sequences.binary_search(&entry.record.sequence).is_err());
        if self.selected.as_ref().is_some_and(|selected| {
            !self
                .entries
                .iter()
                .any(|entry| &entry.key == selected.read(cx).key())
        }) {
            self.clear_selection(cx);
        }
        // Sequence remains the stream high-water mark even after all rows expire.
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| self.matches(entry).then_some(index))
            .collect();
        if following {
            self.scroll.scroll_to_bottom();
        } else {
            self.scroll
                .0
                .borrow()
                .base_handle
                .set_offset(gpui_kit::point(
                    offset.x,
                    (offset.y + px(ROW_HEIGHT * removed_above as f32)).min(px(0.)),
                ));
        }
        cx.notify();
    }
    fn install_snapshot(
        &mut self,
        snapshot: yss_logging::LogSubscriptionDto,
        lease: LogLease,
        cx: &mut Context<Self>,
    ) {
        let same_stream = self.stream == snapshot.stream_id;
        let following = self.following();
        let offset = self.scroll.0.borrow().base_handle.offset();
        let top = (-f32::from(offset.y) / ROW_HEIGHT).floor().max(0.) as usize;
        let anchor = self
            .visible
            .get(top)
            .and_then(|index| self.entries.get(*index))
            .map(|entry| entry.record.sequence);
        let mut previous: std::collections::BTreeMap<_, _> = std::mem::take(&mut self.entries)
            .into_iter()
            .filter(|_| same_stream)
            .map(|entry| (entry.record.sequence, entry))
            .collect();
        self.connecting = false;
        self.stream = snapshot.stream_id;
        self.sequence = snapshot.latest_sequence;
        self.entries = snapshot
            .entries
            .into_iter()
            .rev()
            .take(VIEW_CAPACITY)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|record| {
                previous
                    .remove(&record.sequence)
                    .unwrap_or_else(|| Rc::new(LogEntry::new(record)))
            })
            .collect();
        self.refilter();
        if !following {
            let index = anchor.filter(|_| same_stream).and_then(|sequence| {
                self.visible
                    .iter()
                    .position(|index| self.entries[*index].record.sequence == sequence)
            });
            let y = index
                .map(|index| offset.y + px((top as f32 - index as f32) * ROW_HEIGHT))
                .unwrap_or(px(0.));
            self.scroll
                .0
                .borrow()
                .base_handle
                .set_offset(gpui_kit::point(offset.x, y));
        }
        self.lease = Some(lease);
        cx.notify();
    }

    fn recover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.lease = None;
        if self.recovery_attempts < 3 {
            self.recovery_attempts += 1;
            self.connecting = true;
            cx.defer_in(window, |view, window, cx| view.connect(window, cx));
        } else {
            self.connecting = false;
        }
        cx.notify();
    }
}
