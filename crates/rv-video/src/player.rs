//! Background decoding for playback: a thread decodes ahead into a small bounded queue; the UI
//! takes frames for its clock and requests seeks. Frames carry the seek generation they belong
//! to, so frames decoded before a seek are recognised and dropped.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError, TrySendError};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{Frame, VideoStream};

/// Frames decoded ahead of the play position.
const QUEUE: usize = 6;

enum Command {
    Seek { time: f64, generation: u64 },
    Quit,
}

pub enum Event {
    Opened {
        width: u32,
        height: u32,
        duration: f64,
        frame_rate: f64,
    },
    Frame {
        generation: u64,
        frame: Frame,
    },
    EndOfStream {
        generation: u64,
    },
    Error(String),
}

pub struct Player {
    commands: Sender<Command>,
    events: Receiver<Event>,
    generation: u64,
    thread: Option<JoinHandle<()>>,
}

impl Player {
    /// Opens `path` on a decoding thread; the first frame is decoded from `start`.
    pub fn open(path: PathBuf, start: f64) -> Self {
        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::sync_channel(QUEUE);
        let thread = std::thread::Builder::new()
            .name("video-decode".into())
            .spawn(move || run(&path, start, &command_rx, &event_tx))
            .ok();
        Self {
            commands,
            events,
            generation: 0,
            thread,
        }
    }

    /// Restarts decoding at `time`. Frames still queued from before carry an older generation
    /// than [`Player::generation`] and should be dropped by the caller.
    pub fn seek(&mut self, time: f64) {
        self.generation += 1;
        let _ = self.commands.send(Command::Seek {
            time,
            generation: self.generation,
        });
    }

    /// Generation of the most recent seek request.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Next event without blocking.
    pub fn poll(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Quit);
        // Unblock a pending send so the thread sees the quit command.
        while self.events.try_recv().is_ok() {}
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    path: &std::path::Path,
    start: f64,
    commands: &Receiver<Command>,
    events: &SyncSender<Event>,
) {
    let mut stream = match VideoStream::open(path) {
        Ok(s) => s,
        Err(e) => {
            let _ = events.send(Event::Error(e.to_string()));
            return;
        }
    };
    let opened = Event::Opened {
        width: stream.width,
        height: stream.height,
        duration: stream.duration,
        frame_rate: stream.frame_rate(),
    };
    if events.send(opened).is_err() {
        return;
    }

    let mut generation = 0;
    let mut pending = Some(to_event(stream.seek(start), generation));
    let mut at_end = false;
    loop {
        // Commands first, so seeks are not stuck behind a full queue.
        let command = if at_end && pending.is_none() {
            commands.recv().ok()
        } else {
            match commands.try_recv() {
                Ok(c) => Some(c),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        };
        match command {
            Some(Command::Quit) => return,
            Some(Command::Seek {
                time,
                generation: g,
            }) => {
                generation = g;
                at_end = false;
                pending = Some(to_event(stream.seek(time), generation));
                continue;
            }
            None if at_end && pending.is_none() => return, // command channel closed
            None => {}
        }

        if pending.is_none() && !at_end {
            pending = Some(to_event(stream.next_frame(), generation));
        }
        if let Some(event) = pending.take() {
            if matches!(event, Event::EndOfStream { .. } | Event::Error(_)) {
                at_end = true;
            }
            match events.try_send(event) {
                Ok(()) => {}
                Err(TrySendError::Full(event)) => {
                    pending = Some(event);
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
    }
}

fn to_event(result: std::io::Result<Option<Frame>>, generation: u64) -> Event {
    match result {
        Ok(Some(frame)) => Event::Frame { generation, frame },
        Ok(None) => Event::EndOfStream { generation },
        Err(e) => Event::Error(e.to_string()),
    }
}
