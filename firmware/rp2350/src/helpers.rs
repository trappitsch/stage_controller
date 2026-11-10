//! Helpers for the firmware.

/// Send error message via defmt if `is_err` is true.
pub fn if_err_defmt_msg(is_err: bool, msg: &str) {
    if is_err {
        defmt::error!("{}", msg);
    }
}
