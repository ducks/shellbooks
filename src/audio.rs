use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The audio backend. Owns the rodio output stream + sink for the lifetime
/// of the program; tracks are appended to the same sink so transitions
/// are seamless.
///
/// Speed-change strategy for v0: rodio's built-in `speed()` factor, which
/// changes pitch along with tempo. A future branch will swap in tdpsola
/// for pitch-preserving stretch — that's a bigger DSP project (tdpsola
/// works block-by-block and isn't a drop-in rodio Source).
pub struct Player {
    /// User's preferred speed (0.5 - 3.0). Applied to every new source
    /// appended to the sink, since rodio's speed adjustment is per-source.
    pub speed: f32,
    pub state: PlayerState,
    pub queue: Vec<PathBuf>,
    pub queue_index: usize,
    /// Position within the currently playing file, kept in sync with the
    /// sink's reported position on every `tick()`.
    pub position: Duration,

    // Held for the lifetime of the program. Dropping _stream stops audio.
    _stream: OutputStream,
    sink: Sink,

    /// When the current file started playing (monotonic clock). Used to
    /// derive position since rodio's Sink doesn't expose a direct
    /// "current time" the way we need.
    play_started_at: Option<Instant>,
    /// Accumulated playback time at the moment we last paused. On
    /// resume we reset play_started_at and add this offset.
    paused_offset: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Idle,
    Playing,
    Paused,
}

impl Player {
    pub fn new(initial_speed: f32) -> Result<Self> {
        let stream = OutputStreamBuilder::open_default_stream()
            .context("failed to open default audio output")?;
        let sink = Sink::connect_new(stream.mixer());

        Ok(Self {
            speed: initial_speed.clamp(0.5, 3.0),
            state: PlayerState::Idle,
            queue: vec![],
            queue_index: 0,
            position: Duration::ZERO,
            _stream: stream,
            sink,
            play_started_at: None,
            paused_offset: Duration::ZERO,
        })
    }

    /// Load a fresh queue and start playing at `start_index`.
    /// `offset` is a position within that file (resume from saved progress).
    pub fn play(
        &mut self,
        queue: Vec<PathBuf>,
        start_index: usize,
        offset: Duration,
    ) -> Result<()> {
        self.sink.stop();
        self.queue = queue;
        self.queue_index = start_index.min(self.queue.len().saturating_sub(1));

        // Append from start_index onward. Each file gets the current speed.
        for path in self.queue.iter().skip(self.queue_index) {
            append_file(&self.sink, path, self.speed, offset_for_first(&path, offset))?;
        }
        // The offset only applies to the first appended file.
        self.sink.play();
        self.state = PlayerState::Playing;
        self.position = offset;
        self.paused_offset = offset;
        self.play_started_at = Some(Instant::now());
        Ok(())
    }

    pub fn toggle_pause(&mut self) {
        match self.state {
            PlayerState::Playing => {
                // Capture how far we've advanced since the last resume,
                // then add it to paused_offset so position() stays right.
                if let Some(start) = self.play_started_at.take() {
                    let advanced = start.elapsed().mul_f32(self.speed);
                    self.paused_offset += advanced;
                    self.position = self.paused_offset;
                }
                self.sink.pause();
                self.state = PlayerState::Paused;
            }
            PlayerState::Paused => {
                self.sink.play();
                self.play_started_at = Some(Instant::now());
                self.state = PlayerState::Playing;
            }
            PlayerState::Idle => {}
        }
    }

    pub fn stop(&mut self) {
        self.sink.stop();
        self.state = PlayerState::Idle;
        self.queue.clear();
        self.queue_index = 0;
        self.position = Duration::ZERO;
        self.paused_offset = Duration::ZERO;
        self.play_started_at = None;
    }

    pub fn set_speed(&mut self, speed: f32) {
        let clamped = speed.clamp(0.5, 3.0);
        // Capture current real-time position before changing speed,
        // then resume with the new factor so position math stays right.
        if matches!(self.state, PlayerState::Playing)
            && let Some(start) = self.play_started_at.take()
        {
            self.paused_offset += start.elapsed().mul_f32(self.speed);
            self.play_started_at = Some(Instant::now());
        }
        self.speed = clamped;
        self.sink.set_speed(clamped);
    }

    /// Called from the event loop ~10x/sec. Updates `position` from
    /// the wall clock so the UI can render a progress bar.
    pub fn tick(&mut self) {
        if matches!(self.state, PlayerState::Playing)
            && let Some(start) = self.play_started_at
        {
            self.position = self.paused_offset + start.elapsed().mul_f32(self.speed);
        }
    }

    /// Skip forward/back by `delta` within the current file. Negative is
    /// backward. Crude implementation: rebuild the queue from the new
    /// offset.
    pub fn seek(&mut self, delta: Duration, forward: bool) -> Result<()> {
        if self.queue.is_empty() {
            return Ok(());
        }
        let new_position = if forward {
            self.position + delta
        } else {
            self.position.saturating_sub(delta)
        };
        let queue = self.queue.clone();
        let idx = self.queue_index;
        self.play(queue, idx, new_position)
    }
}

fn offset_for_first(_path: &std::path::Path, offset: Duration) -> Duration {
    offset
}

fn append_file(sink: &Sink, path: &PathBuf, speed: f32, skip: Duration) -> Result<()> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let decoder = Decoder::new(BufReader::new(file))
        .with_context(|| format!("decoding {}", path.display()))?;

    if skip.is_zero() {
        sink.append(decoder.speed(speed));
    } else {
        sink.append(decoder.skip_duration(skip).speed(speed));
    }
    Ok(())
}
