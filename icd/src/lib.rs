//! Shared ICD for stage controller.
//!
//! TODO:
//! - Add `measurements` as a feature that we can ultimately use on the host, mostly to position.
//! - Impl math on position.
//! - Information types to query firmware that is currently running.

#![no_std]

pub mod commands2d;
