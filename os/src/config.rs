#![allow(dead_code)]

pub const BOOTSTRAP_CPU_ID: usize = 0;
pub const CPU_NUM: usize =  4;
pub const LAST_CPU_ID: usize = CPU_NUM - 1;
pub const IS_SINGLE_CORE: bool = false;

pub const KERNEL_HEAP_SIZE: usize = 0x40_0000; // 4 MB
pub const USER_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const MAX_APP_NUM: usize = 4;
pub const APP_BASE_ADDRESS: usize = 0x80400000;
pub const APP_SIZE_LIMIT: usize = 0x20000;

/*
#[cfg(feature = "board_k210")]
pub const CLOCK_FREQ: usize = 403000000 / 62;

#[cfg(feature = "board_qemu")]
pub const CLOCK_FREQ: usize = 12500000;
*/
pub use crate::board::CLOCK_FREQ;
