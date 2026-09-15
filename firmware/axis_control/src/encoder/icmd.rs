//! Encoder quadrature counter driver for the iC-MD.
//!
//! Note: This driver expects one iC-MD chip per axis. Shared encoders between axis are not supported.
//! We will thus always read from `cnt_0`.

use embedded_hal_async::spi::SpiDevice;
use ic_md::FullDeviceStatus;
pub use ic_md::{CntCfg, CntDirection, CntSetup, CntZSignal, IcMdAsync};

use crate::encoder::{EncoderDriver, EncoderError};

/// iC-MD driver that provides a [`EncoderDriver`] compatible interface for reading an axis' status.
pub struct IcMdDriver<SPI: SpiDevice> {
    /// The driver itself.
    driver: IcMdAsync<SPI>,
}

impl<SPI: SpiDevice> IcMdDriver<SPI> {
    /// Get a new iCMD encoder reader with the given settings.
    ///
    /// This will set the given counter setup to the iC-MD and initialize it.
    pub async fn try_new(icmd: IcMdAsync<SPI>, config: CntCfg) -> Result<Self, EncoderError> {
        let mut driver = Self { driver: icmd };
        driver.set_config(config).await?;

        Ok(driver)
    }
}

impl<SPI: SpiDevice> EncoderDriver for IcMdDriver<SPI> {
    type Config = CntCfg;
    type Status = FullDeviceStatus;

    async fn get_position(&mut self) -> Result<i64, EncoderError> {
        let counter_values = self
            .driver
            .read_counter()
            .await
            .map_err(|_| EncoderError::CommunicationError)?;
        let cnt_0 = counter_values
            .get_cnt0()
            .expect("counter 0 is always available");
        Ok(cnt_0)
    }

    async fn reset(&mut self) -> Result<(), EncoderError> {
        self.driver
            .reset_all_counters()
            .await
            .map_err(|_| EncoderError::CommunicationError)
    }

    async fn set_config(&mut self, config: Self::Config) -> Result<(), EncoderError> {
        self.driver.set_counter_config(config);
        self.driver
            .init()
            .await
            .map_err(|_| EncoderError::CommunicationError)
    }

    async fn get_status(&mut self) -> Result<Self::Status, EncoderError> {
        self.driver
            .get_full_device_status()
            .await
            .map_err(|_| EncoderError::CommunicationError)
    }
}
