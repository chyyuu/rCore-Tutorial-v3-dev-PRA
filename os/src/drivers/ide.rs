use crate::config::PAGE_SIZE;

pub const MAX_PAGES: usize = 2535;
pub const IDE_SIZE: usize = MAX_PAGES * PAGE_SIZE;

#[repr(align(4096))]
struct IDE {
    pub data: [u8; IDE_SIZE],
}

static mut IDE: IDE = IDE {
    data: [0; IDE_SIZE],
};

pub fn ide_valid(idx: usize) -> bool {
    idx < MAX_PAGES
}

pub fn ide_read(idx: usize, dst: &mut [u8]) -> usize {
    if !ide_valid(idx) {
        return 1;
    }
    let base = idx * PAGE_SIZE;
    unsafe {
        let ide_ptr = &IDE.data[base..(base+PAGE_SIZE)];
        dst.copy_from_slice(ide_ptr);
    }
    0
}

pub fn ide_write(idx: usize, src: &[u8]) -> usize {
    if !ide_valid(idx) {
        return 1;
    }
    let base = idx * PAGE_SIZE;
    unsafe {
        let ide_ptr = &mut IDE.data[base..(base+PAGE_SIZE)];
        ide_ptr.copy_from_slice(src);
    }
    0
}

#[allow(unused)]
pub fn ide_test() {
    println!("ide_test");
    unsafe {
        IDE.data[0] = 1;
        println!("{}", IDE.data[0]);
    }
}
