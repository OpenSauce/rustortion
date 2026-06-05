use crossbeam::channel::{Receiver, Sender, bounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Capacity of the RT-drop channel. Sized generously so that a burst of stage
/// edits / IR swaps retired in a single `handle_messages` drain doesn't fill it
/// while the background drop thread is briefly descheduled.
const RT_DROP_CAPACITY: usize = 64;

/// Handle for sending objects off the RT thread for deallocation.
///
/// Held by the `Engine` (RT side). A background thread receives and
/// drops these objects, keeping deallocations off the RT thread.
pub struct RtDropHandle {
    drop_tx: Sender<Box<dyn Send>>,
    /// Count of objects leaked (not dropped) because the channel was full or
    /// disconnected. Non-zero means the background drop thread couldn't keep up
    /// and memory was intentionally leaked to preserve the no-dealloc-on-RT
    /// guarantee.
    leaked: Arc<AtomicU64>,
}

/// Receiving end of the RT drop channel.
///
/// Objects received through this channel are deallocated (dropped) on the
/// background thread that owns this receiver, keeping those deallocations
/// off the real-time audio thread.
pub struct RtDropReceiver {
    drop_rx: Receiver<Box<dyn Send>>,
}

impl RtDropHandle {
    /// Create a paired handle and receiver for RT-safe object disposal.
    pub fn new() -> (Self, RtDropReceiver) {
        let (drop_tx, drop_rx) = bounded(RT_DROP_CAPACITY);
        (
            Self {
                drop_tx,
                leaked: Arc::new(AtomicU64::new(0)),
            },
            RtDropReceiver { drop_rx },
        )
    }

    /// Send an object to be dropped on a background thread.
    /// Uses `try_send` to never block the RT thread.
    ///
    /// If the channel is full (drop thread fell behind) or disconnected (drop
    /// thread gone), the object is **leaked** rather than dropped here. Dropping
    /// it would run the allocator's `free` on the RT thread, reintroducing
    /// exactly the deallocation this mechanism exists to eliminate. The leak is
    /// surfaced via [`leaked`](Self::leaked) so it isn't silent.
    pub fn retire(&self, value: Box<dyn Send>) {
        if let Err(err) = self.drop_tx.try_send(value) {
            std::mem::forget(err.into_inner());
            self.leaked.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Number of objects leaked because the drop channel was full or
    /// disconnected. Zero in normal operation; non-zero indicates the
    /// background drop thread couldn't keep up.
    pub fn leaked(&self) -> u64 {
        self.leaked.load(Ordering::Relaxed)
    }
}

impl RtDropReceiver {
    /// Block forever, deallocating objects as they arrive.
    /// Returns when the channel disconnects (i.e. the `RtDropHandle`
    /// was dropped, signalling shutdown).
    pub fn run(&self) {
        while let Ok(value) = self.drop_rx.recv() {
            drop(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn retire_sends_value_to_receiver() {
        let (handle, rx) = RtDropHandle::new();
        let boxed: Box<dyn Send> = Box::new(42_i32);
        handle.retire(boxed);
        // Verify the value arrived
        assert!(rx.drop_rx.try_recv().is_ok());
    }

    #[test]
    fn retire_leaks_instead_of_dropping_when_full() {
        struct DropCounter(Arc<AtomicUsize>);
        impl Drop for DropCounter {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Keep the receiver alive so failures are `Full`, not `Disconnected`.
        let (handle, _rx) = RtDropHandle::new();
        let drops = Arc::new(AtomicUsize::new(0));
        let overflow = 5;

        for _ in 0..(RT_DROP_CAPACITY + overflow) {
            let boxed: Box<dyn Send> = Box::new(DropCounter(drops.clone()));
            handle.retire(boxed);
        }

        // The overflow items were leaked (forgotten), not dropped on this
        // thread. The first `RT_DROP_CAPACITY` are still queued in the channel
        // (alive), so nothing has been deallocated yet.
        assert_eq!(handle.leaked(), overflow as u64);
        assert_eq!(
            drops.load(Ordering::Relaxed),
            0,
            "overflow items must be leaked, not deallocated on the RT thread"
        );
    }
}
