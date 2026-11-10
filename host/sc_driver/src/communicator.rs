//! Stage communicator module.
//!
//! TODO: Reset address to `None` when a request error happens?

use std::{pin::pin, time::Duration};

use ergot::{
    Address, socket::HeaderMessage, toolkits::nusb_v0_1::RouterStack, well_known::ErgotPingEndpoint,
};
use icd::commands2d::{CurrentPositionTopic, Position, StageMoveCmd, StageMoveEndpoint};
use thiserror::Error;
use tokio::time::Instant;

use crate::{address::DeviceAdress, detect};

pub const MTU: u16 = 1024;
pub const OUT_BUFFER_SIZE: usize = 4096;

/// Stage communicator errors.
#[derive(Debug, Error)]
pub enum StageCommunicatorError {
    /// No device is available -> wait for discovery.
    #[error("No device available")]
    NoDeviceAvailable,
    /// Ergot request response error.
    #[error("ReqResponseError")]
    ReqResponseError,
}

/// This is the stage communicator structure.
#[derive(Clone)]
pub struct StageCommunicator {
    pub stack: RouterStack,
    pub timeout: Duration,
}

impl StageCommunicator {
    /// Create a new stage communicator and start the regular find_device task in the background.
    ///
    /// Panics if more than one stage communicator with the same serial number is detected. This
    /// should never be the case and is considered a bug!
    pub fn new_from_serial(serial_number: &str, timeout: Duration) -> Self {
        let stack = RouterStack::new();

        let _ = tokio::task::spawn(detect::register_selected_device(
            stack.clone(),
            serial_number.into(),
        ));

        Self { stack, timeout }
    }

    /// Create a new stage communicator when you know exactly one is connected.
    ///
    /// Panics if zero or more than one stage communicator are detected. It is more prudent to use
    /// [`StageCommunicator::new_from_serial`] and provide the serial number of your communicator
    /// than to use this function if you want to avoid a possible panic.
    pub fn new_single_device(timeout: Duration) -> Self {
        let stack = RouterStack::new();
        let _ = tokio::task::spawn(detect::register_single_device(stack.clone()));

        Self { stack, timeout }
    }

    /// Wait until a device is available and then return its address.
    ///
    /// If no device is available, we wait for the timeout length to see if it shows up again.
    pub async fn wait_for_address_discovery(&self) -> Result<Address, StageCommunicatorError> {
        let tic = Instant::now();
        loop {
            if let Some(addr) = DeviceAdress::get() {
                break Ok(addr);
            } else {
                if Instant::now() - tic > Duration::from_secs(3) {
                    break Err(StageCommunicatorError::NoDeviceAvailable);
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

    /// Stage movement commands.
    pub async fn stage_move(&self, cmd: &StageMoveCmd) -> Result<(), StageCommunicatorError> {
        let addr = self.wait_for_address_discovery().await?;

        let client = self
            .stack
            .endpoints()
            .client::<StageMoveEndpoint>(addr, Some("command2d/stage_move"));
        client
            .request(&cmd)
            .await
            .map_err(|_| StageCommunicatorError::ReqResponseError)
    }

    // /// Home the stage
    // pub async fn home(&self) -> Result<Position, StageCommunicatorError> {
    //     let addr = self.wait_for_address_discovery().await?;

    //     let client = self
    //         .stack
    //         .endpoints()
    //         .client::<StageMoveEndpoint>(addr, Some("command2d/stage_move"));
    //     client
    //         .request(&StageMoveCmd::Home)
    //         .await
    //         .map_err(|_| StageCommunicatorError::ReqResponseError)
    // }

    // /// Move to a new, absolute position.
    // pub async fn move_absolute(&self, pos: Position) -> Result<Position, StageCommunicatorError> {
    //     let addr = self.wait_for_address_discovery().await?;

    //     let client = self
    //         .stack
    //         .endpoints()
    //         .client::<StageMoveEndpoint>(addr, Some("command2d/stage_move"));
    //     client
    //         .request(&StageMoveCmd::Absolute(pos))
    //         .await
    //         .map_err(|_| StageCommunicatorError::ReqResponseError)
    // }

    // /// Move relative to the current position.
    // pub async fn move_relative(&self, pos: Position) -> Result<Position, StageCommunicatorError> {
    //     let addr = self.wait_for_address_discovery().await?;

    //     let client = self
    //         .stack
    //         .endpoints()
    //         .client::<StageMoveEndpoint>(addr, Some("command2d/stage_move"));
    //     client
    //         .request(&StageMoveCmd::Relative(pos))
    //         .await
    //         .map_err(|_| StageCommunicatorError::ReqResponseError)
    // }

    /// Pin the device.
    pub async fn ping(&self) -> Result<HeaderMessage<u32>, StageCommunicatorError> {
        let addr = self.wait_for_address_discovery().await?;

        self.stack
            .endpoints()
            .request_full::<ErgotPingEndpoint>(addr, &42, None)
            .await
            .map_err(|_| StageCommunicatorError::ReqResponseError)
    }

    /// Get a subscriber to the current positioning topic.
    ///
    /// FIXME: This is really ugly
    pub async fn get_current_position_subber(&self) -> Position {
        let subber = self
            .stack
            .topics()
            .heap_bounded_receiver::<CurrentPositionTopic>(64, None);
        let subber = pin!(subber);
        let mut sub = subber.subscribe();

        let msg = sub.recv().await;
        println!("HeaderMessage: {:?}", msg);
        sub.recv().await.t
    }
}
