//! SPSC event channel for thread-safe live parameter updates.
//!
//! ```no_run
//! use klattsch_core::{FormantSynth, ParamUpdate};
//! # #[cfg(feature = "live-events")]
//! # {
//! use klattsch_core::live::{event_channel, SynthEvent};
//!
//! let mut synth = FormantSynth::new(48_000);
//! let (mut tx, mut rx) = event_channel(256);
//!
//! tx.try_send(SynthEvent::SetTarget {
//!     target: ParamUpdate { f0: Some(220.0), ..Default::default() },
//!     transition_samples: 480,
//! }).ok();
//!
//! let mut buf = [0.0f32; 256];
//! synth.drain_events(&mut rx);
//! synth.process(&mut buf);
//! # }
//! ```

use rtrb::{Consumer, Producer, RingBuffer};

use crate::params::ParamUpdate;
use crate::synth::FormantSynth;

/// An event sent to a `FormantSynth` on the audio thread.
#[derive(Clone, Copy, Debug)]
pub enum SynthEvent {
    /// Sparse parameter update with a linear ramp.
    SetTarget {
        target: ParamUpdate,
        transition_samples: u32,
    },
}

/// Sending half of the event channel.
pub struct EventSender {
    producer: Producer<SynthEvent>,
}

impl EventSender {
    /// Try to send an event. Returns `Err(event)` if the ring is full.
    #[allow(clippy::result_large_err)]
    pub fn try_send(&mut self, event: SynthEvent) -> Result<(), SynthEvent> {
        self.producer
            .push(event)
            .map_err(|rtrb::PushError::Full(e)| e)
    }

    pub fn slots_free(&self) -> usize {
        self.producer.slots()
    }
}

/// Receiving half of the event channel.
pub struct EventReceiver {
    consumer: Consumer<SynthEvent>,
}

impl EventReceiver {
    pub fn try_recv(&mut self) -> Option<SynthEvent> {
        self.consumer.pop().ok()
    }
}

/// Paired sender/receiver with the given capacity in events.
pub fn event_channel(capacity: usize) -> (EventSender, EventReceiver) {
    let (producer, consumer) = RingBuffer::new(capacity);
    (EventSender { producer }, EventReceiver { consumer })
}

impl FormantSynth {
    /// Drain all pending events from `receiver` and apply them.
    pub fn drain_events(&mut self, receiver: &mut EventReceiver) {
        while let Some(event) = receiver.try_recv() {
            match event {
                SynthEvent::SetTarget {
                    target,
                    transition_samples,
                } => {
                    self.set_target(target, transition_samples);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    fn vowel_target() -> ParamUpdate {
        ParamUpdate {
            f0: Some(220.0),
            voicing: Some(1.0),
            a1: Some(1.0),
            a2: Some(0.9),
            a3: Some(0.7),
            ..Default::default()
        }
    }

    #[test]
    fn try_send_succeeds_until_full() {
        let (mut tx, mut _rx) = event_channel(4);
        for _ in 0..4 {
            assert!(tx
                .try_send(SynthEvent::SetTarget {
                    target: ParamUpdate::default(),
                    transition_samples: 1,
                })
                .is_ok());
        }
        let result = tx.try_send(SynthEvent::SetTarget {
            target: ParamUpdate::default(),
            transition_samples: 1,
        });
        assert!(result.is_err(), "5th send should fail (capacity 4)");
    }

    #[test]
    fn drain_applies_set_target() {
        let mut synth = FormantSynth::new(48_000);
        let (mut tx, mut rx) = event_channel(16);
        tx.try_send(SynthEvent::SetTarget {
            target: vowel_target(),
            transition_samples: 1,
        })
        .unwrap();
        synth.drain_events(&mut rx);
        let mut buf = [0.0f32; 1];
        synth.process(&mut buf);
        let p = synth.current();
        assert_eq!(p.f0, 220.0);
        assert_eq!(p.voicing, 1.0);
    }

    #[test]
    fn cross_thread_event_delivery() {
        // Producer thread queues events; audio thread (this thread) drains
        // and renders. Verifies the channel is Send-safe.
        use std::thread;
        use std::time::Duration;

        let mut synth = FormantSynth::new(48_000);
        let (mut tx, mut rx) = event_channel(64);

        let producer = thread::spawn(move || {
            for _ in 0..10 {
                tx.try_send(SynthEvent::SetTarget {
                    target: vowel_target(),
                    transition_samples: 1,
                })
                .unwrap();
                thread::sleep(Duration::from_micros(100));
            }
        });

        // Drain in a loop until producer is done and ring is empty.
        let mut buf = [0.0f32; 256];
        let mut received = 0;
        for _ in 0..1000 {
            synth.drain_events(&mut rx);
            synth.process(&mut buf);
            // Count is hard to observe directly; just verify we don't deadlock
            // and the synth's state eventually reflects the update.
            received += 1;
            if synth.current().f0 == 220.0 {
                break;
            }
        }
        producer.join().unwrap();
        // After all events drained, current target should match.
        synth.drain_events(&mut rx);
        synth.process(&mut buf);
        assert_eq!(synth.current().f0, 220.0);
        let _ = received;
        let _ = Params::DEFAULT;
    }
}
