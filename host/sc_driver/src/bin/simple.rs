//! A simple interface for the driver.
//!
//! Run with.
//! `cargo r --bin simple`

use std::time::Duration;

use icd::commands2d::{Position, StageMoveCmd};
use sc_driver::communicator::StageCommunicator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let driver = StageCommunicator::new_single_device(Duration::from_secs(1));

    loop {
        println!("\nEnter command:");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        println!("");

        let cmd = Commands::from(input.as_str());
        match cmd {
            Commands::MoveAbsolute { x, y } => {
                driver.stage_move(&cmd.into()).await.unwrap();
                println!("Moving absolute: x: {x}, y: {y}")
            }
            Commands::MoveRelative { x, y } => {
                driver.stage_move(&cmd.into()).await.unwrap();
                println!("Moving relative: x: {x}, y: {y}")
            }
            Commands::Home => {
                driver.stage_move(&cmd.into()).await.unwrap();
                println!("Homing the axes...");
            }
            Commands::Stop => {
                driver.stage_move(&cmd.into()).await.unwrap();
                println!("Stop the axes!");
            }
            Commands::Help => help(),
            Commands::InvalidCommand => println!("Invalid command. Press `h` to get help."),
            Commands::Quit => break,
        }
    }
    println!("Done.");
    Ok(())
}

/// The commands that we have.
#[derive(Debug)]
enum Commands {
    Help,
    InvalidCommand,
    MoveAbsolute { x: i32, y: i32 },
    MoveRelative { x: i32, y: i32 },
    Home,
    Stop,
    Quit,
}

impl From<Commands> for StageMoveCmd {
    fn from(value: Commands) -> Self {
        match value {
            Commands::MoveAbsolute { x, y } => StageMoveCmd::Absolute(Position::new(x, y)),
            Commands::MoveRelative { x, y } => StageMoveCmd::Relative(Position::new(x, y)),
            Commands::Home => StageMoveCmd::Home,
            Commands::Stop => StageMoveCmd::Stop,
            _ => panic!("Should never be called with {value:?}"),
        }
    }
}

impl From<&str> for Commands {
    fn from(value: &str) -> Self {
        let value = value.trim().to_lowercase();
        let mut parts = value.split(" ");

        match parts.next() {
            Some("ma") => {
                let x = parse_next(parts.next());
                let y = parse_next(parts.next());
                if x.is_none() || y.is_none() {
                    Commands::InvalidCommand
                } else {
                    Commands::MoveAbsolute {
                        x: x.unwrap(),
                        y: y.unwrap(),
                    }
                }
            }
            Some("mr") => {
                let x = parse_next(parts.next());
                let y = parse_next(parts.next());
                if x.is_none() || y.is_none() {
                    Commands::InvalidCommand
                } else {
                    Commands::MoveRelative {
                        x: x.unwrap(),
                        y: y.unwrap(),
                    }
                }
            }
            Some("i") => Self::Home,
            Some("s") => Self::Stop,
            Some("h") => Self::Help,
            Some("q") => Self::Quit,
            _ => Self::InvalidCommand,
        }
    }
}

/// Parse the next value as u32.
///
/// Returns Some(i32) if there was a value, or None otherwise.
fn parse_next(value: Option<&str>) -> Option<i32> {
    let val = value?;
    i32::from_str_radix(val, 10).ok()
}

/// Print a help string to the stdin
fn help() {
    println!(
        "Help:

Below are the available commands for the simple stage interface.
Whenever X and/or Y are given, provide the X/Y position you want to drive to in µm.
These positions must be given as integers and should always be separated by a space.

ma X Y:   Move absolute. 
mr X Y:   Move relative.
i:        Home the stage (initialize).
s:        Stop both axes.

h:        Show this help.
q:        Quit the program."
    );
}
