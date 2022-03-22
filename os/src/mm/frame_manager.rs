
use super::{PageTable, PhysPageNum, P2V_MAP};
use alloc::vec::Vec;

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

#[allow(unused)]
pub enum PRA {
    FIFO,
    Clock,
}

struct ClockQue {
    ppns: Vec<PhysPageNum>,
    ptr: usize,
}

impl ClockQue {
    fn new() -> Self {
        ClockQue{
            ppns: Vec::new(),
            ptr: 0,
        }
    }
    fn inc(&mut self) {
        if self.ptr == self.ppns.len() - 1 {
            self.ptr = 0;
        }
        else {
            self.ptr += 1;
        }
    }
    pub fn push(&mut self, ppn: PhysPageNum) {
        self.ppns.push(ppn);
    }

    pub fn pop(&mut self, page_table: &mut PageTable) -> Option<PhysPageNum> {
        loop {
            let ppn = self.ppns[self.ptr];
            let vpn = *(P2V_MAP.exclusive_access().get(&ppn).unwrap());
            let pte = page_table.find_pte(vpn).unwrap();
            if !pte.is_valid() {
                panic!("[kernel] PAGE FAULT: Pte not valid in PRA Clock pop.");
            }
            if !pte.accessed() {
                self.ppns.remove(self.ptr);
                if self.ptr == self.ppns.len() {
                    self.ptr = 0;
                }
                return Some(ppn);
            }
            pte.change_access();
            // println!("change pte access.");
            if pte.accessed() {
                panic!("[kernel] PAGE FAULT: Pte access did not change.");
            }
            self.inc();
        }
    }
}

pub struct LocalFrameManager {
    used_pra: PRA,
    fifo_que: Queue<PhysPageNum>,
    clock_que: ClockQue,
}

impl LocalFrameManager {
    pub fn new(pra: PRA) -> Self {
        LocalFrameManager{
            used_pra: pra,
            fifo_que: Queue::new(),
            clock_que: ClockQue::new(),
        }
    }
    pub fn get_next_frame(&mut self, page_table: &mut PageTable) -> Option<PhysPageNum> {
        match self.used_pra {
            PRA::FIFO => {
                self.fifo_que.pop()
            }
            PRA::Clock => {
                self.clock_que.pop(page_table)
            }
        }
    }
    pub fn insert_frame(&mut self, ppn: PhysPageNum) {
        match self.used_pra {
            PRA::FIFO => {
                self.fifo_que.push(ppn)
            }
            PRA::Clock => {
                self.clock_que.push(ppn)
            }
        }
    }
}
