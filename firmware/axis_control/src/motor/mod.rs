//! Motor control for one axis.

pub mod tmc2209;

/// Direction of a motor.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Move the motor in positive direction, according to definition of the axis.
    Positive,
    /// Move the motor in negative direction, according to definition of the axis.
    Negative,
}

/// This is the trait that every motor driver must implement to be compatible with our motor struct.
pub trait MotorDriver {
    type Config;
    type Status;

    /// Set the configuration of the Motor.
    #[must_use]
    fn set_config(&mut self, config: Self::Config) -> impl Future<Output = Result<(), MotorError>>;

    /// Move the motor in a given direction by a certain number of steps.
    #[must_use]
    fn move_relative(
        &mut self,
        steps: u64,
        dir: Direction,
    ) -> impl Future<Output = Result<(), MotorError>>;

    /// Get the status of the motor driver.
    #[must_use]
    fn get_status(&mut self) -> impl Future<Output = Result<Self::Status, MotorError>>;
}

/// Errors that a motor might return.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg(feature = "defmt")]
#[derive(defmt::Format)]
#[non_exhaustive]
pub enum MotorError {
    /// Communication with the driver failed.
    CommunicationError,
    /// An error occured when setting an output pin.
    OutputPinError,
    /// A value that was supposed to be set is out of range.
    RangeError,
}

/// Motor struct that controls a given motor.
pub struct Motor<M: MotorDriver> {
    pub motor_driver: M,
}

impl<M: MotorDriver> Motor<M> {
    /// Create a new motor with a given MotorDriver.
    pub fn new(motor_driver: M) -> Self {
        Self { motor_driver }
    }
}
