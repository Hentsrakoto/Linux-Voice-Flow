//! Nonblocking native-event ingress and FIFO dispatch, independent of Tauri.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, RecvTimeoutError, Sender},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::hotkey::HotkeyEvent;

pub(crate) type ShortcutMessage = (String, HotkeyEvent);

/// The native X11 callback must never wait for the application's manager lock:
/// registration can itself be waiting for that same X11 event thread.
#[derive(Clone)]
pub(crate) struct ShortcutRelay(pub(crate) Sender<ShortcutMessage>);

impl ShortcutRelay {
    pub(crate) fn send(&self, shortcut: String, event: HotkeyEvent) {
        // Unbounded only for small key-state tuples; no audio or dictated text.
        // A disconnected receiver means the application is shutting down.
        let _ = self.0.send((shortcut, event));
    }
}

pub(crate) fn spawn_relay_worker<F>(
    receiver: Receiver<ShortcutMessage>,
    stopping: Arc<AtomicBool>,
    mut handle: F,
) -> JoinHandle<()>
where
    F: FnMut(ShortcutMessage, &AtomicBool) + Send + 'static,
{
    thread::spawn(move || {
        while !stopping.load(Ordering::Acquire) {
            match receiver.recv_timeout(Duration::from_millis(20)) {
                Ok(message) => handle(message, &stopping),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn relay_producer_does_not_wait_for_blocked_consumer_and_keeps_fifo() {
        let (sender, receiver) = mpsc::channel();
        let relay = ShortcutRelay(sender);
        let stopping = Arc::new(AtomicBool::new(false));
        let (entered_sender, entered_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let (seen_sender, seen_receiver) = mpsc::channel();
        let mut first = true;
        let worker = spawn_relay_worker(receiver, Arc::clone(&stopping), move |message, _| {
            if first {
                first = false;
                entered_sender.send(()).unwrap();
                release_receiver
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap();
            }
            seen_sender.send(message).unwrap();
        });
        relay.send("Ctrl+Alt+Space".into(), HotkeyEvent::Pressed);
        entered_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        // A synchronous producer would wait here and fail the bounded signal.
        let (sent_sender, sent_receiver) = mpsc::channel();
        let producer = thread::spawn(move || {
            relay.send("Ctrl+Alt+Space".into(), HotkeyEvent::Released);
            sent_sender.send(()).unwrap();
        });
        let sent_without_consumer = sent_receiver.recv_timeout(Duration::from_secs(1)).is_ok();
        release_sender.send(()).unwrap();
        assert!(sent_without_consumer, "native callback blocked on consumer");
        assert_eq!(
            seen_receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .1,
            HotkeyEvent::Pressed
        );
        assert_eq!(
            seen_receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .1,
            HotkeyEvent::Released
        );
        stopping.store(true, Ordering::Release);
        producer.join().unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn idle_relay_worker_stops_without_waiting_for_sender_disconnect() {
        let (_sender, receiver) = mpsc::channel();
        let stopping = Arc::new(AtomicBool::new(false));
        let (exited_sender, exited_receiver) = mpsc::channel();
        let worker = spawn_relay_worker(receiver, Arc::clone(&stopping), |_, _| {
            panic!("unexpected event");
        });
        stopping.store(true, Ordering::Release);
        let joiner = thread::spawn(move || {
            worker.join().unwrap();
            exited_sender.send(()).unwrap();
        });
        exited_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        joiner.join().unwrap();
    }
}
