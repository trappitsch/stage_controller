//! Stage specific commands.
//!
//! These are all the commands specific for stage movement, specifically for a two dimensional (XY)
//! stage.

use ergot::{endpoint, topic};
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

endpoint!(StageMoveEndpoint, StageMoveCmd, (), "command2d/stage_move");

topic!(CurrentPositionTopic, Position, "command2d/current_position");

/// Move commands to move a two axis stage.
#[derive(Serialize, Deserialize, Schema, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum StageMoveCmd {
    /// Move to a new position.
    Absolute(Position),
    /// Move relative by a given amount, stored as position.
    Relative(Position),
    /// Home the stage.
    Home,
    /// Stop the movement.
    Stop,
}

/// Axis.
#[derive(Serialize, Deserialize, Schema, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Axis {
    /// The x axis of the 2D stage.
    X,
    /// The y axis of the 2D stage.
    Y,
}

/// Position in micrometers.
#[derive(Serialize, Deserialize, Schema, Default, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Position {
    pub x_um: i32,
    pub y_um: i32,
}

impl Position {
    pub fn new(x_um: i32, y_um: i32) -> Self {
        Position { x_um, y_um }
    }
}
