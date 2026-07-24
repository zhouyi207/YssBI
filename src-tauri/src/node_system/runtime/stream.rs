use super::CancellationToken;
use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidStreamCapacity;

impl fmt::Display for InvalidStreamCapacity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded stream capacity must be greater than zero")
    }
}

impl std::error::Error for InvalidStreamCapacity {}

#[derive(Debug, PartialEq, Eq)]
pub enum StreamSendError<T> {
    Full(T),
    Cancelled(T),
    Closed(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamReceiveError {
    Empty,
    Cancelled,
    Closed,
}

struct ChannelState<T> {
    queue: VecDeque<T>,
    sender_count: usize,
    receiver_count: usize,
    closed: bool,
}

struct Channel<T> {
    capacity: usize,
    state: Mutex<ChannelState<T>>,
    not_empty: Arc<Condvar>,
    not_full: Arc<Condvar>,
    cancellation: CancellationToken,
}

impl<T> Channel<T> {
    fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.closed = true;
        self.not_empty.notify_all();
        self.not_full.notify_all();
    }

    fn is_closed(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .closed
    }
}

pub struct BoundedStreamSender<T> {
    channel: Arc<Channel<T>>,
}

impl<T> Clone for BoundedStreamSender<T> {
    fn clone(&self) -> Self {
        let mut state = self
            .channel
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.sender_count += 1;
        drop(state);
        Self {
            channel: self.channel.clone(),
        }
    }
}

impl<T> Drop for BoundedStreamSender<T> {
    fn drop(&mut self) {
        let mut state = self
            .channel
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.sender_count -= 1;
        self.channel.not_empty.notify_all();
    }
}

impl<T> BoundedStreamSender<T> {
    /// Blocks while the bounded channel is full. Cancellation and closure wake it.
    pub fn send(&self, value: T) -> Result<(), StreamSendError<T>> {
        let mut value = Some(value);
        let mut state = self
            .channel
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        loop {
            if self.channel.cancellation.is_cancelled() {
                return Err(StreamSendError::Cancelled(value.take().unwrap()));
            }
            if state.closed || state.receiver_count == 0 {
                return Err(StreamSendError::Closed(value.take().unwrap()));
            }
            if state.queue.len() < self.channel.capacity {
                state.queue.push_back(value.take().unwrap());
                self.channel.not_empty.notify_one();
                return Ok(());
            }
            state = self
                .channel
                .not_full
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    pub fn try_send(&self, value: T) -> Result<(), StreamSendError<T>> {
        let mut state = self
            .channel
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if self.channel.cancellation.is_cancelled() {
            return Err(StreamSendError::Cancelled(value));
        }
        if state.closed || state.receiver_count == 0 {
            return Err(StreamSendError::Closed(value));
        }
        if state.queue.len() == self.channel.capacity {
            return Err(StreamSendError::Full(value));
        }
        state.queue.push_back(value);
        self.channel.not_empty.notify_one();
        Ok(())
    }

    pub fn close(&self) {
        self.channel.close();
    }

    pub fn is_closed(&self) -> bool {
        self.channel.is_closed()
    }
}

pub struct BoundedStreamReceiver<T> {
    channel: Arc<Channel<T>>,
}

impl<T> Clone for BoundedStreamReceiver<T> {
    fn clone(&self) -> Self {
        let mut state = self
            .channel
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.receiver_count += 1;
        drop(state);
        Self {
            channel: self.channel.clone(),
        }
    }
}

impl<T> Drop for BoundedStreamReceiver<T> {
    fn drop(&mut self) {
        let mut state = self
            .channel
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.receiver_count -= 1;
        self.channel.not_full.notify_all();
    }
}

impl<T> BoundedStreamReceiver<T> {
    /// Blocks until a value arrives, cancellation occurs, or the stream closes.
    pub fn recv(&self) -> Result<T, StreamReceiveError> {
        let mut state = self
            .channel
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        loop {
            if let Some(value) = state.queue.pop_front() {
                self.channel.not_full.notify_one();
                return Ok(value);
            }
            if self.channel.cancellation.is_cancelled() {
                return Err(StreamReceiveError::Cancelled);
            }
            if state.closed || state.sender_count == 0 {
                return Err(StreamReceiveError::Closed);
            }
            state = self
                .channel
                .not_empty
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    pub fn try_recv(&self) -> Result<T, StreamReceiveError> {
        let mut state = self
            .channel
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(value) = state.queue.pop_front() {
            self.channel.not_full.notify_one();
            return Ok(value);
        }
        if self.channel.cancellation.is_cancelled() {
            return Err(StreamReceiveError::Cancelled);
        }
        if state.closed || state.sender_count == 0 {
            return Err(StreamReceiveError::Closed);
        }
        Err(StreamReceiveError::Empty)
    }

    pub fn close(&self) {
        self.channel.close();
    }

    pub fn is_closed(&self) -> bool {
        self.channel.is_closed()
    }

    pub(crate) fn same_channel(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.channel, &other.channel)
    }
}

pub fn bounded_stream_channel<T>(
    capacity: usize,
    cancellation: CancellationToken,
) -> Result<(BoundedStreamSender<T>, BoundedStreamReceiver<T>), InvalidStreamCapacity> {
    if capacity == 0 {
        return Err(InvalidStreamCapacity);
    }
    let not_empty = Arc::new(Condvar::new());
    let not_full = Arc::new(Condvar::new());
    cancellation.register_waiter(&not_empty);
    cancellation.register_waiter(&not_full);
    let channel = Arc::new(Channel {
        capacity,
        state: Mutex::new(ChannelState {
            queue: VecDeque::with_capacity(capacity),
            sender_count: 1,
            receiver_count: 1,
            closed: false,
        }),
        not_empty,
        not_full,
        cancellation,
    });
    Ok((
        BoundedStreamSender {
            channel: channel.clone(),
        },
        BoundedStreamReceiver { channel },
    ))
}
