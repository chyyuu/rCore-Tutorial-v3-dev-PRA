use super::__switch;
use super::add_task;
use super::{fetch_task, TaskStatus};
use super::{ProcessControlBlock, TaskContext, TaskControlBlock};
use crate::trap::TrapContext;
use crate::arch::get_cpu_id;
use crate::config::CPU_NUM;
use spin::Mutex;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;

pub struct Processor {
    current: Option<Arc<TaskControlBlock>>,
    idle_task_cx: TaskContext,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    pub static ref PROCESSOR: Vec<Mutex<Processor>> = {
        let mut tmp = Vec::new();
        for _ in 0..CPU_NUM{
            tmp.push(Mutex::new(Processor::new()))
        }
        tmp
    };
}

pub fn run_tasks() {
    loop {
        let id = get_cpu_id();
        let mut processor = PROCESSOR[id].lock();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_lock();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
            let mut processor = PROCESSOR[id].lock();
            if let Some(next_task) = processor.take_current() {
                let inner = next_task.inner_lock();
                if inner.task_status == TaskStatus::Ready{
                    drop(inner);
                    add_task(next_task);
                }
            }
            drop(processor);
        }
    }
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR[get_cpu_id()].lock().take_current()
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR[get_cpu_id()].lock().current()
}

pub fn current_process() -> Arc<ProcessControlBlock> {
    current_task().unwrap().process.upgrade().unwrap()
}

pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_lock()
        .get_trap_cx()
}

pub fn current_trap_cx_user_va() -> usize {
    current_task()
        .unwrap()
        .inner_lock()
        .res
        .as_ref()
        .unwrap()
        .trap_cx_user_va()
}

pub fn current_kstack_top() -> usize {
    current_task().unwrap().kstack.get_top()
}

pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR[get_cpu_id()].lock();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}
