#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{get_time, mmap};

fn random(m: isize, last: isize) -> isize {
    let mut u = (last * 942137 + 99995423) % (m * 2);
    if u < 0 {
        u = -u;
    }
    u - m
}

#[no_mangle]
fn main() -> i32 {
    let start: usize = 0x10000000;
    let len: usize = 4096 * 100;
    let prot: usize = 3;
    let seed: isize = get_time();
    let mut v: isize = seed;
    assert_eq!(0, mmap(start, len, prot));
    for i in 0..len {
        let mut k = i as isize + random(4096, v);
        while k<0 || k>=len as isize {
            k = i as isize + random(4096, k);
        }
        let addr: *mut u8 = (start + k as usize) as *mut u8;
        unsafe {
            *addr = (k as usize) as u8;
        }
        v = k;
    }

    let mut v: isize = seed;
    for i in 0..len {
        let mut k = i as isize + random(4096, v);
        while k<0 || k>=len as isize {
            k = i as isize + random(4096, k);
        }
        let addr: *mut u8 = (start + k as usize) as *mut u8;
        unsafe {
            assert_eq!(*addr, (k as usize) as u8);
        }
        v = k;
    }
    println!("Test3 OK!");
    0
}
