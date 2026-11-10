//! Motor control for one axis.

pub mod pio_programs;
pub mod tmc2209;

/// Axis identification.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, defmt::Format)]
pub enum AxisId {
    X,
    Y,
    Z,
}

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

    /// Get axis
    fn get_axis_id(&self) -> AxisId;

    /// Set the configuration of the Motor.
    #[must_use]
    fn set_config(&mut self, config: Self::Config) -> impl Future<Output = Result<(), MotorError>>;

    /// Enable/disable the motor.
    #[must_use]
    fn set_enabled(&mut self, enabled: bool) -> impl Future<Output = Result<(), MotorError>>;

    /// Move the motor in a given direction by a certain number of steps.
    ///
    /// Note that this does not enable or disable the motor. If necessary, you need to manually enable/disable the motors.
    #[must_use]
    fn move_steps(
        &mut self,
        steps: u32,
        dir: Direction,
    ) -> impl Future<Output = Result<(), MotorError>>;

    /// Get the status of the motor driver.
    #[must_use]
    fn get_status(&mut self) -> impl Future<Output = Result<Self::Status, MotorError>>;
}

/// Errors that a motor might return.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
