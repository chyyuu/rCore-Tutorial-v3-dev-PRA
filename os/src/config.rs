#![allow(dead_code)]

pub const BOOTSTRAP_CPU_ID: usize = 0;
pub const CPU_NUM: usize =  4;
pub const LAST_CPU_ID: usize = CPU_NUM - 1;

pub const KERNEL_HEAP_SIZE: usize = 0x40_0000; // 4 MB
