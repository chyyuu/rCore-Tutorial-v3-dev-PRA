use crate::config::CLOCK_FREQ;
use crate::arch::set_timer;
use riscv::register::time;
use core::convert::TryInto;

const TICKS_PER_SEC: usize = 10;
const MSEC_PER_SEC: usize = 1000;

pub fn get_time() -> usize {
    time::read()
}

pub fn get_time_ms() -> usize {
    time::read() / (CLOCK_FREQ / MSEC_PER_SEC)
}

pub fn set_next_trigger() {
    set_timer((get_time() + CLOCK_FREQ / TICKS_PER_SEC).try_into().unwrap());
}
