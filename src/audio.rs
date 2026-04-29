use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;

/// Wraps rodio's Sink with a tdpsola-based time-stretch pipe so speed
/// changes preserve pitch. Held in App. Only one player active at a time.
pub struct Player {
    /// Speed factor (1.0 = normal). Driven by user; passed through to the
    /// stretcher when chunks are produced.
    pub speed: f32,
    pub state: PlayerState,
    pub queue: Vec<PathBuf>,
    pub queue_index: usize,
    /// Position within the currently playing file.
    pub position: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Idle,
    Playing,
    Paused,
}

impl Player {
    pub fn new(initial_speed: f32) -> Result<Self> {
        Ok(Self {
            speed: initial_speed.clamp(0.5, 3.0),
            state: PlayerState::Idle,
            queue: vec![],
            queue_index: 0,
            position: Duration::ZERO,
        })
    }

    /// Load and start a queue of files (chapters or whole book), starting
    /// at the given file index and offset within that file.
    pub fn play(&mut self, _queue: Vec<PathBuf>, _start_index: usize, _offset: Duration) -> Result<()> {
        // TODO: build a rodio source per file, wrap each in a tdpsola
        // stretcher driven by self.speed, append to a Sink on a held
        // OutputStream. Update state.
        self.state = PlayerState::Playing;
        Ok(())
    }

    pub fn toggle_pause(&mut self) {
        self.state = match self.state {
            PlayerState::Playing => PlayerState::Paused,
            PlayerState::Paused => PlayerState::Playing,
            PlayerState::Idle => PlayerState::Idle,
        };
    }

    pub fn stop(&mut self) {
        self.state = PlayerState::Idle;
        self.queue.clear();
        self.position = Duration::ZERO;
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.5, 3.0);
        // TODO: feed the new factor into the live stretcher.
    }

    pub fn seek(&mut self, _position: Duration) -> Result<()> {
        // TODO: skip the current source forward/back. Easier when each
        // chapter is its own file — for single-file books we'll need to
        // re-decode from the offset.
        Ok(())
    }
}
