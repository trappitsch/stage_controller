//! All the things to handle communication with the main task.

use defmt::warn;
use embassy_time::{Duration, Timer};
use icd::commands2d::{CurrentPositionTopic, Position};

use crate::STACK;

#[embassy_executor::task]
pub async fn current_position_broadcast() {
    Timer::after(Duration::from_secs(3)).await;
    loop {
        Timer::after(Duration::from_secs(5)).await;
        warn!("Sending broadcast message");
        let _ = STACK
            .topics()
            .broadcast::<CurrentPositionTopic>(&Position::default(), None);
    }
}
