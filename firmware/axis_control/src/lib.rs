//! Main crate to control one single axis for a stage controller.
//!
//! The whole stage controller crate is MCU independent. It is a `no_std` crate that uses
//! [`embedded_hal`] and [`embedded_hal_async`] traits for interacting with the hardware.
//!
//! An axis consists of:
//! - A motor driver (TMC2209).
//! - A linear encoder that is read by an iC-MD.
//! - Two end switches (probably optional).

#![no_std]

pub mod encoder;
pub mod motor;

use motor::{Motor, MotorDriver};

/// Control one stage axis.
pub struct AxisControl<M: MotorDriver> {
    motor: Motor<M>,
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
    pub async fn move_steps(&mut self, steps: u64, dir: motor::Direction) {
        self.motor
            .motor_driver
            .move_relative(steps, dir)
            .await
            .unwrap();
    }
}
