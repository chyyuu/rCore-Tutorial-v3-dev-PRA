mod cpu;
mod sbi;
pub mod stdout;

pub use sbi::{
    set_timer,
    shutdown,
};

core::arch::global_asm!(include_str!("boot/entry.asm"));

pub fn get_cpu_id() -> usize {
    cpu::id()
}
