//! Encoder control for one axis.

pub mod icmd;

/// Trait to implement for every new encoder/encoder reader.
pub trait EncoderDriver {
    type Config;
    type Status;

    /// Get the current number of steps in the encoder.
    #[must_use]
    fn get_position(&mut self) -> impl Future<Output = Result<i64, EncoderError>>;

    /// Reset the counter to zero.
    #[must_use]
    fn reset(&mut self) -> impl Future<Output = Result<(), EncoderError>>;

    /// Set the configuration of the encoder/encoder reader.
    ///
    /// This trait must set the configuration of a given encoder/encoder reader and initialize this
    /// same device before completing.
    #[must_use]
    fn set_config(
        &mut self,
        config: Self::Config,
    ) -> impl Future<Output = Result<(), EncoderError>>;

    /// Get the status of the encoder.
    #[must_use]
    fn get_status(&mut self) -> impl Future<Output = Result<Self::Status, EncoderError>>;
}

/// Errors that an encoder might return.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum EncoderError {
    /// Communication with the encoder failed.
    CommunicationError,
}
