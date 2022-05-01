mod cpu;
mod sbi;
pub mod stdout;

pub use sbi::{
    set_timer,
    shutdown,   
};

core::arch::global_asm!(include_str!("boot/entry.asm"));

/// 需要在堆初始化之后，因为这里 STDOUT 打印需要用到 Mutex 锁，这需要堆分配
pub fn cpu_init(cpu_id: usize) {
    println!("Hello, CPU [{}]", cpu_id);
}

pub fn get_cpu_id() -> usize {
    cpu::id()
}
