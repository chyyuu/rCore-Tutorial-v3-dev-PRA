mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::config::{CPU_NUM};
use crate::arch::get_cpu_id;
use spin::Mutex;
use crate::loader::{get_app_data, get_num_app};
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;

pub struct TaskManager {
    num_app: usize,
    inner: Mutex<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: Vec<TaskControlBlock>,
    current_task: [usize; CPU_NUM],
    cpu_free: usize,
}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
        }
        TaskManager {
            num_app,
            inner: Mutex::new(TaskManagerInner {
                    tasks,
                    current_task: [num_app - 1; CPU_NUM],
                    cpu_free: 0,
                })
        }
    };
}

impl TaskManager {
    fn run_first_task(&self) -> ! {
        let cpu_id = get_cpu_id();
        let mut inner = self.inner.lock();
        if let Some(next) = self.find_next_task(&inner) {
            inner.current_task[cpu_id] = next;
        }
        else{
            inner.cpu_free += 1;
            println!("[kernel] cpu [{}] freed.", cpu_id);
            drop(inner);
            loop{};
        }
        let first = inner.current_task[cpu_id];
        let task0 = &mut inner.tasks[first];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    fn find_next_task(&self, inner: &TaskManagerInner) -> Option<usize> {
        let current = inner.current_task[get_cpu_id()];
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    fn get_current_token(&self) -> usize {
        let inner = self.inner.lock();
        inner.tasks[inner.current_task[get_cpu_id()]].get_user_token()
    }

    fn get_current_trap_cx(&self) -> &'static mut TrapContext {
        let inner = self.inner.lock();
        inner.tasks[inner.current_task[get_cpu_id()]].get_trap_cx()
    }

    fn run_next_task(&self, status:TaskStatus) {
        let cpu_id = get_cpu_id();
        let mut inner = self.inner.lock();
        let current = inner.current_task[cpu_id];
        inner.tasks[current].task_status = status;
        if let Some(next) = self.find_next_task(&inner) {
            let current = inner.current_task[cpu_id];
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task[cpu_id] = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // go back to user mode
        } else {
            inner.cpu_free += 1;
            println!("[kernel] cpu [{}] freed.", cpu_id);
            if inner.cpu_free == CPU_NUM {
                panic!("All applications completed!");
            }
            else {
                drop(inner);
                loop{};
            }
        }
    }
}

pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

pub fn suspend_current_and_run_next() {
    TASK_MANAGER.run_next_task(TaskStatus::Ready);
}

pub fn exit_current_and_run_next() {
    TASK_MANAGER.run_next_task(TaskStatus::Exited);
}

pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}
