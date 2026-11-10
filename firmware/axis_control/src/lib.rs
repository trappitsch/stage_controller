//! Main crate to control one single axis for a stage controller.
//!
//! The whole stage controller crate is MCU independent. It is a `no_std` crate that uses
//! [`embedded_hal`] and [`embedded_hal_async`] traits for interacting with the hardware.
//!
//! An axis consists of:
//! - A motor driver (TMC2209).
//! - A linear encoder that is read by an iC-MD.
//! - Two end switches (probably optional).
//!
//! TODO:
//! - The problem with this is that if the axis holds the motor driver and the encoder,
//!   I can not read the encoder whenever the motor moves via PIO.
//! - This might be fine for now, but let's think hard about this...
//! - When should we actually read the encoder?
//! - We could update the encoder every second when the PIO is moving and we are waiting for it...
//! - Then read it whenever the broadcast wants to read and update some state.
//! - Also update it once the whole move is finished.
//! - Actually: We want to not only update the position it is currently at but rather the whole state of the instrument
//! - The state needs to live in the ICD as we will be able to broadcast it.

#![no_std]

pub mod encoder;
pub mod motor;
pub mod state;
pub mod tasks;

use motor::{Motor, MotorDriver};

use crate::{
    motor::{Direction, MotorError},
    tasks::MoveCommand,
};

/// Position in um for one axis.
///
/// We save the position as full micrometers as an i32. This gives us around +- 2km of range.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Position {
    value_um: i32,
}

impl Position {
    /// Create a new position from a value.
    pub fn new(value_um: i32) -> Self {
        Position { value_um }
    }

    /// Get a tuple of steps and direction.
    pub fn get_steps_and_dir(&self) -> (u32, Direction) {
        let dir = if self.value_um < 0 {
            Direction::Negative
        } else {
            Direction::Positive
        };

        (self.value_um.abs() as u32, dir)
    }
}

/// Control one stage axis.
pub struct AxisControl<M: MotorDriver> {
    pub motor: Motor<M>,
}

impl<M> defmt::Format for AxisControl<M>
where
    M: MotorDriver,
{
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "AxisControl {}",
            &self.motor.motor_driver.get_axis_id()
        );
    }
}

impl<M> AxisControl<M>
where
    M: MotorDriver,
{
    /// Create a new axis controller.
    pub fn new(motor_driver: M) -> Self {
        let motor = Motor::new(motor_driver);
        Self { motor }
    }

    //// TODO: Temporary
    pub async fn move_steps(&mut self, steps: u32, dir: Direction) {
        self.motor
            .motor_driver
            .move_steps(steps, dir)
            .await
            .unwrap();
    }

    pub async fn disable_motor(&mut self) -> Result<(), MotorError> {
        self.motor.motor_driver.set_enabled(false).await
    }

    pub async fn enable_motor(&mut self) -> Result<(), MotorError> {
        self.motor.motor_driver.set_enabled(true).await
    }

    /// Execute a move the stage command.
    ///
    /// Note that you must enable and disable the motor outside of this move command.
    /// The reason for this is that then, this future can be dropped to stop the PIO movement,
    /// however, the motor is still correctly disabled after the stop was issued.
    pub async fn execute_move(&mut self, move_cmd: MoveCommand) -> Result<(), MotorError> {
        // TODO:
        // - Relative motions
        // - Absolute motions
        // - Homing motions
        match move_cmd {
            MoveCommand::MoveRelative(pos) => {
                let (steps, dir) = pos.get_steps_and_dir();
                self.move_steps(steps, dir).await;
                defmt::warn!("here");
            }
            _ => {
                #[cfg(feature = "defmt")]
                defmt::info!("Move command not yet implemented: {}", move_cmd);
            }
        }
        Ok(())
    }
}
