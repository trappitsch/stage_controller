//! All the things to handle communication with the main task.

use core::{any::Any, pin::pin};

use axis_control::{
    AxisControl, Position as AxisPosition,
    motor::{MotorDriver, tmc2209::Tmc2209Driver},
    tasks::{MoveCommand, task_axis_mover},
};
use defmt::warn;
use embassy_rp::{
    gpio::Output,
    peripherals::{PIO0, PIO1},
    uart::BufferedUart,
};
use embassy_time::{Duration, Timer};
use icd::commands2d::{CurrentPositionTopic, Position, StageMoveCmd, StageMoveEndpoint};

use crate::{
    STACK, XAX_MOVE_CHANNEL, XAX_STOP_SIGNAL, YAX_MOVE_CHANNEL, YAX_STOP_SIGNAL,
    helpers::if_err_defmt_msg,
};

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

/// Stage move ergot task.
#[embassy_executor::task]
pub async fn stage_move_listener() {
    let socket = STACK
        .endpoints()
        .bounded_server::<StageMoveEndpoint, 2>(Some("command2d/stage_move"));
    let socket = pin!(socket);
    let mut hdl = socket.attach();

    let xax_move_sender = &XAX_MOVE_CHANNEL.sender();
    let yax_move_sender = &YAX_MOVE_CHANNEL.sender();

    loop {
        let _ = hdl
            .serve(async |cmd| match cmd {
                &StageMoveCmd::Absolute(ref pos) => {
                    let cmd_x = MoveCommand::MoveAbsolute(AxisPosition::new(pos.x_um));
                    let cmd_y = MoveCommand::MoveAbsolute(AxisPosition::new(pos.y_um));
                    if_err_defmt_msg(
                        xax_move_sender.try_send(cmd_x).is_err(),
                        "MovAbsX sending error",
                    );
                    if_err_defmt_msg(
                        yax_move_sender.try_send(cmd_y).is_err(),
                        "MovAbsY sending error",
                    );
                }
                &StageMoveCmd::Relative(ref pos) => {
                    let cmd_x = MoveCommand::MoveRelative(AxisPosition::new(pos.x_um));
                    let cmd_y = MoveCommand::MoveRelative(AxisPosition::new(pos.y_um));
                    if_err_defmt_msg(
                        xax_move_sender.try_send(cmd_x).is_err(),
                        "MovRelX sending error",
                    );
                    Timer::after(Duration::from_millis(100)).await;
                    if_err_defmt_msg(
                        yax_move_sender.try_send(cmd_y).is_err(),
                        "MovRelY sending error",
                    );
                }
                &StageMoveCmd::Home => {
                    if_err_defmt_msg(
                        xax_move_sender.try_send(MoveCommand::Home).is_err(),
                        "HomeX sending error",
                    );
                    if_err_defmt_msg(
                        yax_move_sender.try_send(MoveCommand::Home).is_err(),
                        "HomeY sending error",
                    );
                }
                &StageMoveCmd::Stop => {
                    XAX_STOP_SIGNAL.signal(());
                    YAX_STOP_SIGNAL.signal(());
                }
            })
            .await;
    }
}

/// Hardware-locked task that spawns the x axis mover.
#[embassy_executor::task]
pub async fn x_axis_mover(
    axis: AxisControl<Tmc2209Driver<'static, Output<'static>, BufferedUart, PIO0, 0>>,
) {
    task_axis_mover(axis, &XAX_MOVE_CHANNEL, &XAX_STOP_SIGNAL).await;
}

/// Hardware-locked task that spawns the y axis mover.
#[embassy_executor::task]
pub async fn y_axis_mover(
    axis: AxisControl<Tmc2209Driver<'static, Output<'static>, BufferedUart, PIO1, 0>>,
) {
    task_axis_mover(axis, &YAX_MOVE_CHANNEL, &YAX_STOP_SIGNAL).await;
}
