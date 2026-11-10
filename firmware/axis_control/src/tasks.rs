//! Tasks and associated commands for a single axis.

use embassy_futures::select;
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, ThreadModeRawMutex},
    channel::Channel,
    signal::Signal,
};

use crate::{AxisControl, Position, motor::MotorDriver};

/// Command channel type for sending move commands to a specified axis.
pub type AxisMoveChannel = Channel<ThreadModeRawMutex, MoveCommand, 32>;

/// Stop signal that will make an axis stop its current motion.
pub type AxisStopSignal = Signal<CriticalSectionRawMutex, ()>;

/// Commands that can be sent to the stage.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MoveCommand {
    /// Home the axis: Drive to end switch and then set the current position to zero.
    Home,
    /// Move to a new absolute position.
    MoveAbsolute(Position),
    /// Move relative to the current position.
    MoveRelative(Position),
}

/// The task to move the axis.
///
/// This should be called from an embassy task with a concrete type and then be polled in an endless loop.
pub async fn task_axis_mover<M: MotorDriver>(
    mut axis: AxisControl<M>,
    move_channel: &'static AxisMoveChannel,
    stop_signal: &'static AxisStopSignal,
) {
    let move_recv = move_channel.receiver();
    loop {
        // wait for a move command
        let move_cmd = move_recv.receive().await;

        // enable the motors and create the future to start the move.
        let _ = axis.enable_motor().await;
        let move_fut = axis.execute_move(move_cmd);

        // reset the stop signal, then await the future OR a stop signal (clear all existing stop signals).
        let _ = select::select(stop_signal.wait(), move_fut).await;

        // reset the move signals (ignore whatever came while we were moving) and disable the motor.
        move_recv.clear();
        let _ = axis.disable_motor().await;

        defmt::info!("ax_mov_tsk: disabled motor {}", &axis);
    }
}
