pub mod buffer;
mod error;
pub mod mixer;
pub mod rechanneler;
pub mod resampler;
pub mod session;
pub mod source;

#[cfg(feature = "wav")]
pub mod wav;

use crate::source::{Sample, Source};
use crate::session::{ChannelCount, SampleRate};

/// A basic sound-playing object. When fed to an output stream, will play the samples it contains until it has no more.
/// If the samples have a different sample rate than the output stream, the output will sound sped up or slowed down.
/// Use a resampler (such as boop::resampler::Polyphase, or implement your own) to resample it at the correct rate.
pub struct Player {
    samples: Box<[Sample]>,
    channels: ChannelCount,
    sample_rate: SampleRate,
    offset: usize,
}

impl Player {
    pub fn new(channels: ChannelCount, sample_rate: SampleRate, samples: Box<[Sample]>) -> Self {
        Self { channels, sample_rate, samples, offset: 0 }
    }
}

impl Source for Player {
    #[inline]
    fn channel_count(&self) -> ChannelCount {
        self.channels
    }

    #[inline]
    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn write_samples(&mut self, buffer: &mut [Sample]) -> usize {
        let old_offset = self.offset;
        self.offset += buffer.len();
        if let Some(i) = self.samples.get(old_offset..self.offset) {
            buffer.copy_from_slice(i);
            buffer.len()
        } else if let Some(i) = self.samples.get(old_offset..) {
            buffer[..i.len()].copy_from_slice(i);
            i.len()
        } else {
            0
        }
    }
}
