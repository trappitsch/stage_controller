//! Detect devices for a given netstack and print the info.
//!
//! Run with:
//!

use std::time::Duration;

use icd::commands2d::Position;
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

    // a absolute motion
    let mabs_pos = comm.move_absolute(new_pos.clone()).await.unwrap();
    assert_eq!(&mabs_pos, &new_pos);

    // a relative motion
    let mrel_pos = comm.move_relative(new_pos.clone()).await.unwrap();
    assert_eq!(&mrel_pos, &new_pos);

    // homing command
    let home_pos = comm.home().await.unwrap();
    assert_eq!(home_pos, Position::default());
}
