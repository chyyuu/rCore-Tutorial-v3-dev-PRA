#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{get_time, mmap};

fn random(m: usize, last: usize) -> usize {
    (last * 942137 + 99995423) % m
}

#[no_mangle]
fn main() -> i32 {
    let start: usize = 0x10000000;
    let len: usize = 4096 * 400;
    let prot: usize = 3;
    let seed: usize = get_time() as usize;
    let mut v: usize = seed;
    assert_eq!(0, mmap(start, len, prot));
    for i in 0..64 {
        for _j in 0..64 {
            let k = random(len / 64, v) + len / 64 * i;
            let addr: *mut u8 = (start + k) as *mut u8;
            unsafe {
                *addr = (k) as u8;
            }
            v = k;
        }
    }

    let mut v: usize = seed;
    for i in 0..64 {
        for _j in 0..64 {
            let k = random(len / 64, v) + len / 64 * i;
            let addr: *mut u8 = (start + k as usize) as *mut u8;
            unsafe {
                assert_eq!(*addr, (k as usize) as u8);
            }
            v = k;
        }
    }
    println!("Test5 OK!");
    0
}
