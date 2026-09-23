//! Stage specific commands.
//!
//! These are all the commands specific for stage movement, specifically for a two dimensional (XY)
//! stage.

use ergot::{endpoint, topic};
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

endpoint!(HomeEndpoint, (), Position, "command2d/home");

endpoint!(
    MoveAbsEndpoint,
    Position,
    Position,
    "command2d/move_absolute"
);

endpoint!(
    MoveRelEndpoint,
    Position,
    Position,
    "command2d/move_relative"
);

topic!(CurrentPositionTopic, Position, "command2d/current_position");

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
    pub x_um: i64,
    pub y_um: i64,
}
