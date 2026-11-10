//! Detect devices for a given netstack and print the info.
//!
//! Run with:
//!

use std::time::Duration;

use icd::commands2d::{Position, StageMoveCmd};
use sc_driver::communicator::StageCommunicator;

#[tokio::main]
async fn main() {
    env_logger::init();

    let comm = StageCommunicator::new_single_device(Duration::from_secs(1));

    let new_pos = Position {
        x_um: 1234,
        y_um: 5678,
    };

    println!("{:?}", comm.ping().await);

    // an absolute motion
    let cmd = StageMoveCmd::Absolute(new_pos.clone());
    comm.stage_move(&cmd).await.unwrap();

    // a relative motion
    let cmd = StageMoveCmd::Relative(new_pos.clone());
    comm.stage_move(&cmd).await.unwrap();

    // homing command
    comm.stage_move(&StageMoveCmd::Home).await.unwrap();
}
