use crossbeam::channel::{Receiver, Sender, bounded};

/// Handle for sending objects off the RT thread for deallocation.
///
/// Held by the `Engine` (RT side). A background thread receives and
/// drops these objects, keeping deallocations off the RT thread.
pub struct RtDropHandle {
    drop_tx: Sender<Box<dyn Send>>,
}

/// Receiving end of the RT drop channel.
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
    /// Drain and drop all objects waiting in the channel.
    pub fn drain(&self) {
        while self.drop_rx.try_recv().is_ok() {}
    }

    /// Block until one value arrives, then drain any remaining.
    /// Returns `false` when the channel is disconnected (shutdown).
    pub fn recv_and_drain(&self) -> bool {
        if self.drop_rx.recv().is_ok() {
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
