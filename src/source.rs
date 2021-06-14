use crate::session::{ChannelCount, SampleRate};

pub type Sample = f32;

/// Trait for a source of audio that outputs PCM at a given sample rate.
pub trait Source {
    /// Returns the number of channels in this `Source`.
    ///
    /// This function must always return the same value.
    fn channel_count(&self) -> ChannelCount;

    /// Returns the sample rate the written data should be interpreted at.
    ///
    /// This function must always return the same value.
    fn sample_rate(&self) -> SampleRate;

    /// Writes the next set of samples to an output `buffer`.
    ///
    /// The implementor is expected to "remember" its progress through the sound it's playing,
    /// such that it must continue where it left off on subsequent calls to this function.
    /// If there are multiple channels then the samples must be interleaved.
    /// Values must be written contiguously from the start of the `buffer`.
    ///
    /// Returns the number of samples which were written to the `buffer`.
    /// A value lower than `buffer.len()` indicates the sound has ended.
    /// Any further calls to this function after that must not write anything and return 0.
    fn write_samples(&mut self, buffer: &mut [Sample]) -> usize;
}
