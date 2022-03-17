
use super::{VirtPageNum, PhysPageNum, MapType, frame_alloc};
use crate::task::current_process;
use crate::drivers::{MAX_PAGES, ide_read, ide_write};
use crate::sync::UPSafeCell;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;

#[derive(Debug)]
pub struct Queue<T> {
    data: Vec<T>,
}

impl <T> Queue<T> {
    pub fn new() -> Self {
        Queue{ data: Vec::new() }
    }

    pub fn push(&mut self, item: T) {
        self.data.push(item);
    }

    pub fn pop(&mut self) ->Option<T> {
        let l = self.data.len();
        if l > 0 {
            let v = self.data.remove(0);
            Some(v)
        } else {
            None
        }
    }
}

pub struct IdeManager {
    current: usize,
    end: usize,
    recycled: Vec<usize>,
    map: BTreeMap<VirtPageNum, usize>,
}

impl IdeManager {
    pub fn new() -> Self {
        Self {
            current: 0,
            end: MAX_PAGES - 1,
            recycled: Vec::new(),
            map: BTreeMap::new(),
        }
    }
    pub fn swap_in(&mut self, vpn: VirtPageNum, src: &mut [u8]) {
        if let Some(idx) = self.recycled.pop() {
            ide_write(idx, src);
            self.map.insert(vpn, idx);
        } else if self.current == self.end {
            panic!("[kernel] Virtual disk space is not enough for handling page fault.");
        } else {
            self.current += 1;
            ide_write(self.current - 1, src);
            self.map.insert(vpn, self.current - 1);
        }
    }
    pub fn swap_out(&mut self, vpn: VirtPageNum, dst: &mut [u8]) {
        let idx = self.map.get(&vpn).unwrap();
        ide_read(*idx, dst);
        self.recycled.push(*idx);
        self.map.remove(&vpn);
    }
    pub fn check(&mut self, vpn: VirtPageNum) -> bool {
        self.map.get(&vpn) != None
    }
}

lazy_static! {
    pub static ref FRAME_QUE: Arc<UPSafeCell<Queue<PhysPageNum>>> =
        Arc::new(unsafe { UPSafeCell::new( Queue::new()) });
    pub static ref P2V_MAP: Arc<UPSafeCell<BTreeMap<PhysPageNum, VirtPageNum>>> =
        Arc::new(unsafe { UPSafeCell::new( BTreeMap::new()) });
    pub static ref IDE_MANAGER: Arc<UPSafeCell<IdeManager>> =
        Arc::new(unsafe { UPSafeCell::new( IdeManager::new()) });
}

pub fn do_pgfault(addr: usize) -> bool {
    let process = current_process();
    let mut pcb = process.inner_exclusive_access();
    let memory_set = &mut pcb.memory_set;
    let vpn: VirtPageNum = addr.into();
    // println!("{}", addr);
    for area in &mut memory_set.areas {
        // println!("{} {}", area.vpn_range.get_start().0, area.vpn_range.get_end().0);
        if vpn >= area.vpn_range.get_start() && vpn <= area.vpn_range.get_end() {
            let ppn: PhysPageNum;
            match area.map_type {
                MapType::Identical => {
                    panic!("[kernel] Page fault MapType should not be Identical.");
                }
                MapType::Framed => {
                    let mut ide_manager = IDE_MANAGER.exclusive_access();

                    if let Some(frame) = frame_alloc() { // enough frame
                        ppn = frame.ppn;
                        area.data_frames.insert(vpn, frame);
                    }
                    else {  // need to swap out a frame
                        ppn = FRAME_QUE.exclusive_access().pop().unwrap();
                        let data_old = ppn.get_bytes_array();
                        let p2v_map = P2V_MAP.exclusive_access();
                        let vpn_old = p2v_map.get(&ppn).unwrap();
                        ide_manager.swap_in(*vpn_old, data_old);
                        area.unmap_one(&mut memory_set.page_table, *vpn_old);

                        let frame = frame_alloc().unwrap();
                        area.data_frames.insert(vpn, frame);
                    }
                    
                    if ide_manager.check(vpn) {
                        let data = ppn.get_bytes_array();
                        ide_manager.swap_out(vpn, data);
                    }
                }
            }
            memory_set.page_table.map(vpn, ppn, area.get_flag_bits());
            return true;
        }
    }
    false
}
