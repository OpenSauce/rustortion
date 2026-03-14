use crossbeam::channel::{Receiver, Sender, bounded};

/// Handle for sending objects off the RT thread for deallocation.
///
/// Held by the `Engine` (RT side). A background thread receives and
/// drops these objects, keeping deallocations off the RT thread.
pub struct RtDropHandle {
    drop_tx: Sender<Box<dyn Send>>,
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
        let (drop_tx, drop_rx) = bounded(16);
        (Self { drop_tx }, RtDropReceiver { drop_rx })
    }

    /// Send an object to be dropped on a background thread.
    /// Uses `try_send` to never block the RT thread.
    pub fn retire(&self, value: Box<dyn Send>) {
        let _ = self.drop_tx.try_send(value);
    }
}

impl RtDropReceiver {
    /// Drain and deallocate all objects waiting in the channel.
    pub fn drain(&self) {
        while let Ok(value) = self.drop_rx.try_recv() {
            drop(value);
        }
    }

    /// Block until one object arrives, deallocate it and any others
    /// queued behind it. Returns `false` when the channel disconnects
    /// (i.e. the `RtDropHandle` was dropped, signalling shutdown).
    pub fn recv_and_drain(&self) -> bool {
        if let Ok(value) = self.drop_rx.recv() {
            drop(value);
            self.drain();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retire_sends_value_to_receiver() {
        let (handle, rx) = RtDropHandle::new();
        let boxed: Box<dyn Send> = Box::new(42_i32);
        handle.retire(boxed);
        rx.drain();
    }

    #[test]
    fn retire_does_not_block_when_full() {
        let (handle, _rx) = RtDropHandle::new();
        for i in 0..20 {
            let boxed: Box<dyn Send> = Box::new(i);
            handle.retire(boxed);
        }
    }
}
