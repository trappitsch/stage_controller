//! Stage control motor driver for the TMC2209 motor driver.
//!
//! This supports any of the TMC2209 drivers. Some special ones are noted below:
//!
//! ## SilentStepStick TMC2209
//!
//! The maximum current on this version is set to 1.7A. The jumper on the board must be set such
//! that access via UART is possible.
//! TODO: Jumper details
//!
//! TODO: Next steps:
//! - Set speed when doing move_relative.

use embassy_rp::{Peri, gpio, pio};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    mutex::{Mutex, MutexGuard},
};

use tmc2209_uart::{DrvStatus, Gconf, ReadableRegister, WritableRegister};
pub use tmc2209_uart::{MicrostepResolution, Tmc2209};

use crate::motor::{
    AxisId, Direction, MotorDriver, MotorError,
    pio_programs::{
        PioStepDirProgram,
        pio_step_dir::{PioStepDir, StepDir},
    },
};

/// We wrap our Tmc2209 in a mutex to be able to write to.
pub type Tmc2209Mutex<UART> = Mutex<CriticalSectionRawMutex, Tmc2209<UART>>;
pub type Tmc2209MutexGuard<'a, UART> = MutexGuard<'a, CriticalSectionRawMutex, Tmc2209<UART>>;

/// TMC2209 driver that provides a [`MotorDriver`] compatible interface for controlling an axis.
pub struct Tmc2209Driver<'a, POUT, UART, PIO: pio::Instance, const SM: usize> {
    /// Not enabled pin.
    p_nen: POUT,
    /// The PioStepDir program that can driver our stepper.
    stepper: PioStepDir<'a, PIO, SM>,
    /// [`embassy_sync::mutex::Mutex`] holding our [`Tmc2209`] driver with a UART interface.
    driver: &'a Tmc2209Mutex<UART>,
    /// The address of the driver.
    ///
    /// This is needed as we have to set the driver address whenever we acquire the lock on the
    /// driver, since the UART is shared with up to 4 TMC2209s.
    address: u8,
    /// The last set configuration.
    config: Tmc2209Config,
}

impl<'a, POUT, UART, PIO, const SM: usize> Tmc2209Driver<'a, POUT, UART, PIO, SM>
where
    POUT: embedded_hal::digital::OutputPin,
    UART: embedded_io_async::Read + embedded_io_async::Write,
    PIO: pio::Instance,
{
    /// Get a new TMC2209 motor driver with these settings.
    ///
    /// This sets up a driver and configures it with the given [`Tmc2209Config`].
    pub async fn try_new(
        p_nen: POUT,
        pio_config: PioConfig<'_, 'a, PIO, impl gpio::Pin, SM>,
        driver: &'a Tmc2209Mutex<UART>,
        address: u8,
        config: Tmc2209Config,
    ) -> Result<Self, MotorError> {
        let stepper = pio_config.get_stepper();

        let mut drv = Self {
            p_nen,
            stepper,
            driver,
            address,
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
}

impl<'a, POUT, UART, PIO, const SM: usize> MotorDriver for Tmc2209Driver<'a, POUT, UART, PIO, SM>
where
    POUT: embedded_hal::digital::OutputPin,
    UART: embedded_io_async::Read + embedded_io_async::Write,
    PIO: pio::Instance,
{
    type Config = Tmc2209Config;
    type Status = DrvStatus;

    fn get_axis_id(&self) -> AxisId {
        match self.address {
            0x00 => AxisId::X,
            0x01 => AxisId::Y,
            _ => AxisId::Z,
        }
    }

    async fn set_enabled(&mut self, is_enabled: bool) -> Result<(), MotorError> {
        // set the pin
        if is_enabled {
            self.p_nen
                .set_low()
                .map_err(|_| MotorError::OutputPinError)?;
        } else {
            self.p_nen
                .set_high()
                .map_err(|_| MotorError::OutputPinError)?;
        };

        // set the driver
        self.locked_driver()
            .await
            .set_enabled_async(is_enabled)
            .await
            .map_err(|_| MotorError::CommunicationError)?;
        Ok(())
    }

    async fn move_steps(&mut self, steps: u32, dir: Direction) -> Result<(), MotorError> {
        self.stepper.move_with_dir(steps, dir.into()).await;
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

/// PIO Configuration.
///
/// Sturcture that contains everything required to make a PIO configuration.
pub struct PioConfig<'p, 'a, PIO, GPIOPIN, const SM: usize>
where
    PIO: pio::Instance,
    GPIOPIN: gpio::Pin,
{
    pub p_step: pio::Pin<'a, PIO>,
    pub p_dir: Peri<'a, GPIOPIN>,
    pub program: &'p PioStepDirProgram<'a, PIO>,
    pub irq: pio::Irq<'a, PIO, SM>,
    pub sm: pio::StateMachine<'a, PIO, SM>,
}

impl<'p, 'a, PIO, GPIOPIN, const SM: usize> PioConfig<'p, 'a, PIO, GPIOPIN, SM>
where
    PIO: pio::Instance,
    GPIOPIN: gpio::Pin,
{
    /// Get the stepper instance with a given PIO frequenc
    fn get_stepper(self) -> PioStepDir<'a, PIO, SM> {
        let mut stepper = PioStepDir::new(self.sm, self.p_step, self.p_dir, self.irq, self.program);

        // TODO: Can i change the speed with the frequency (Hz)?
        // What is the max value? The min value we know.
        // Maybe we should do this when we move.
        //
        // ANSWER: Yes: this can change the speed at which the motor runs.
        // Check out the min and max frequency on how they are defined in the PIO programs.
        // This should be the new speed at which we will move around.
        //
        // So: min at the the Cycles68 settings is around 34 and max 2205882.
        // The motor starts stalling at around >20_000 max, so there's no reason to go higher.
        // Minimum setting of one to a few k is probably good.
        //
        // Needs error checking on what the user sets, but otherwise keep it free range and give
        // some docs advice.
        stepper.set_frequency(8_000).unwrap();
        stepper
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

impl From<Direction> for StepDir {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Positive => Self::Forward,
            Direction::Negative => Self::Reverse,
        }
    }
}
