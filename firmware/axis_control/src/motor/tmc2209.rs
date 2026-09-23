//! Stage control motor driver for the TMC2209 motor driver.
//!
//! This supports any of the TMC2209 drivers. Some special ones are noted below:
//!
//! ## SilentStepStick TMC2209
//!
//! The maximum current on this version is set to 1.7A. The jumper on the board must be set such
//! that access via UART is possible.
//! TODO: Jumper details

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    mutex::{Mutex, MutexGuard},
};

use tmc2209_uart::{DrvStatus, Gconf, ReadableRegister, WritableRegister};
pub use tmc2209_uart::{MicrostepResolution, Tmc2209};

use crate::motor::{Direction, MotorDriver, MotorError};

/// We wrap our Tmc2209 in a mutex to be able to write to.
pub type Tmc2209Mutex<UART> = Mutex<CriticalSectionRawMutex, Tmc2209<UART>>;
pub type Tmc2209MutexGuard<'a, UART> = MutexGuard<'a, CriticalSectionRawMutex, Tmc2209<UART>>;

/// TMC2209 driver that provides a [`MotorDriver`] compatible interface for controlling an axis.
pub struct Tmc2209Driver<'a, POUT, UART, D> {
    /// Not enabled pin.
    p_nen: POUT,
    /// OutputPin for `STEP`.
    p_step: POUT,
    /// OutputPin for `DIR`.
    p_dir: POUT,
    /// [`embassy_sync::mutex::Mutex`] holding our [`Tmc2209`] driver with a UART interface.
    driver: &'a Tmc2209Mutex<UART>,
    /// The address of the driver.
    ///
    /// This is needed as we have to set the driver address whenever we acquire the lock on the
    /// driver, since the UART is shared with up to 4 TMC2209s.
    address: u8,
    /// An async delay provider.
    delay: D,
    /// The last set configuration.
    config: Tmc2209Config,
}

impl<'a, POUT, UART, D> Tmc2209Driver<'a, POUT, UART, D>
where
    POUT: embedded_hal::digital::OutputPin,
    UART: embedded_io_async::Read + embedded_io_async::Write,
    D: embedded_hal_async::delay::DelayNs,
{
    /// Get a new TMC2209 motor driver with these settings.
    ///
    /// This sets up a driver and configures it with the given [`Tmc2209Config`].
    pub async fn try_new(
        p_nen: POUT,
        p_step: POUT,
        p_dir: POUT,
        driver: &'a Tmc2209Mutex<UART>,
        address: u8,
        delay: D,
        config: Tmc2209Config,
    ) -> Result<Self, MotorError> {
        let mut drv = Self {
            p_nen,
            p_step,
            p_dir,
            driver,
            address,
            delay,
            config: Default::default(),
        };

        drv.set_config(config).await?;
        Ok(drv)
    }

    /// Obtain a locked driver from the Mutex with the proper address set.
    async fn locked_driver(&mut self) -> Tmc2209MutexGuard<'a, UART> {
        let mut drv = self.driver.lock().await;
        drv.set_slave_addr(self.address);
        drv
    }

    /// Enable or disable a motor.
    async fn set_enabled(&mut self, is_enabled: bool) -> Result<(), MotorError> {
        self.locked_driver()
            .await
            .set_enabled_async(is_enabled)
            .await
            .map_err(|_| MotorError::CommunicationError)?;
        Ok(())
    }

    /// Set the microstep resolution.
    async fn set_microstep_resolution(
        &mut self,
        value: MicrostepResolution,
    ) -> Result<(), MotorError> {
        self.locked_driver()
            .await
            .set_microsteps_async(value)
            .await
            .map_err(|_| MotorError::CommunicationError)?;
        Ok(())
    }

    /// Read a register.
    async fn read_register<R: ReadableRegister>(&mut self) -> Result<R, MotorError> {
        let reg = self
            .locked_driver()
            .await
            .read_register_async::<R>()
            .await
            .map_err(|_| MotorError::CommunicationError)?;
        Ok(reg)
    }

    /// Write a register.
    async fn write_register<W: WritableRegister>(&mut self, reg: &W) -> Result<(), MotorError> {
        self.locked_driver()
            .await
            .write_register_async(reg)
            .await
            .map_err(|_| MotorError::CommunicationError)?;

        Ok(())
    }

    /// Do the given number of steps.
    async fn do_steps(&mut self, steps: u64, delay_ns: u32) -> Result<(), MotorError> {
        self.p_nen
            .set_low()
            .map_err(|_| MotorError::OutputPinError)?;
        self.set_enabled(true).await?;

        for _ in 0..steps {
            self.p_step
                .set_high()
                .map_err(|_| MotorError::OutputPinError)?;
            self.delay.delay_ns(delay_ns).await;

            self.p_step
                .set_low()
                .map_err(|_| MotorError::OutputPinError)?;
            self.delay.delay_ns(delay_ns).await;
        }

        self.set_enabled(false).await?;
        self.p_nen
            .set_high()
            .map_err(|_| MotorError::OutputPinError)?;

        Ok(())
    }
}

impl<'a, POUT, UART, D> MotorDriver for Tmc2209Driver<'a, POUT, UART, D>
where
    POUT: embedded_hal::digital::OutputPin,
    UART: embedded_io_async::Read + embedded_io_async::Write,
    D: embedded_hal_async::delay::DelayNs,
{
    type Config = Tmc2209Config;
    type Status = DrvStatus;

    async fn move_relative(&mut self, steps: u64, dir: Direction) -> Result<(), MotorError> {
        match dir {
            Direction::Positive => self
                .p_dir
                .set_low()
                .map_err(|_| MotorError::OutputPinError)?,
            Direction::Negative => self
                .p_dir
                .set_high()
                .map_err(|_| MotorError::OutputPinError)?,
        }

        let delay_ns = self.config.move_config.get_delay_ns_rough_half_step();
        self.do_steps(steps, delay_ns).await?;

        Ok(())
    }

    async fn set_config(&mut self, config: Self::Config) -> Result<(), MotorError> {
        let mut gconf = Gconf::new();
        gconf.set_mstep_reg_select(true);

        // Invert the motor direction (GCONF_shaft)
        if config.invert {
            gconf.set_shaft(true);
        }

        // write gconf register
        self.write_register(&gconf).await?;

        // set current
        self.locked_driver()
            .await
            .set_current_async(
                config.current.irun,
                config.current.ihold,
                config.current.hold_delay,
            )
            .await
            .map_err(|_| MotorError::CommunicationError)?;

        // set microstep resolution
        self.set_microstep_resolution(config.move_config.microsteps)
            .await?;

        self.set_enabled(false).await?;

        self.config = config;

        Ok(())
    }

    async fn get_status(&mut self) -> Result<Self::Status, MotorError> {
        self.locked_driver()
            .await
            .drv_status_async()
            .await
            .map_err(|_| MotorError::CommunicationError)
    }
}

/// The full configuration of the Tmc2209Driver.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Tmc2209Config {
    /// Set to true to invert the axis.
    pub invert: bool,
    /// Current configuration.
    pub current: CurrentConfig,
    /// Microstep resolution.
    pub move_config: MoveConfig,
}

/// Current configuration.
///
/// If you access the fields directly and enter values above the maximum, the maximum will simply be
/// set. Alternatively, use the `set_` functions that return results and tell you if there was a
/// range error.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CurrentConfig {
    /// Motor run current, 0-31: 0=1/32 max, ..., 31=32/32 max. Default: 31
    pub irun: u8,
    /// Motor hold current, 0-31: 0=1/32 max, ..., 31=32/32 max. Default: 31
    pub ihold: u8,
    /// Hold delay: 0-15; Controls the number of clock cycles for motor power down after standstill is detected.
    /// Instant power down (0), 1..15: Delay per current reduction step in multiples of 2^18 clock
    /// cycles (clock: 12 MHz). Default: 0
    pub hold_delay: u8,
}

impl CurrentConfig {
    /// Set IRun with error checking.
    pub fn set_irun(&mut self, value: u8) -> Result<(), MotorError> {
        if value > 31 {
            Err(MotorError::RangeError)
        } else {
            self.irun = value;
            Ok(())
        }
    }

    /// Set IHold with error checking.
    pub fn set_ihold(&mut self, value: u8) -> Result<(), MotorError> {
        if value > 31 {
            Err(MotorError::RangeError)
        } else {
            self.ihold = value;
            Ok(())
        }
    }

    /// Set IRun with error checking.
    pub fn set_hold_delay(&mut self, value: u8) -> Result<(), MotorError> {
        if value > 31 {
            Err(MotorError::RangeError)
        } else {
            self.hold_delay = value;
            Ok(())
        }
    }
}

impl Default for CurrentConfig {
    fn default() -> Self {
        Self {
            irun: 31,
            ihold: 0,
            hold_delay: 0,
        }
    }
}

/// Movement configuration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MoveConfig {
    /// Microstep resolution configuration.
    pub microsteps: MicrostepResolution,
    /// At what velocity should we do rough movements: in ~ full steps per second.
    pub rough_velocity: u32,
    /// At what velocity should we do the fine movement corrections / small movements: ~ full steps
    /// per second.
    pub fine_velocity: u32,
    /// Movement mode.
    pub movement_mode: MoveMode,
}

impl Default for MoveConfig {
    fn default() -> Self {
        Self {
            microsteps: MicrostepResolution::M8,
            rough_velocity: 1000,
            fine_velocity: 200,
            movement_mode: Default::default(),
        }
    }
}

impl MoveConfig {
    /// Get the nanosecond delay for a half-step in rough velocity.
    fn get_delay_ns_rough_half_step(&self) -> u32 {
        1_000_000_000 / self.rough_velocity / (self.microsteps.microsteps() as u32) / 2
    }

    /// Get the nanosecond delay for a half-step in rough velocity.
    fn get_delay_ns_fine_half_step(&self) -> u32 {
        1_000_000_000 / self.fine_velocity / (self.microsteps.microsteps() as u32) / 2
    }
}

/// Mode of movement.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MoveMode {
    /// Set to stealthchop.
    #[default]
    StealthChop,
    /// Set to spreadcycle chopper.
    SpreadCycleChopper,
}
