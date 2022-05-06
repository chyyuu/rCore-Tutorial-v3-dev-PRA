mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::config::{MAX_APP_NUM, CPU_NUM};
use crate::loader::{get_num_app, init_app_cx};
use crate::arch::get_cpu_id;
use spin::Mutex;
use lazy_static::*;
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;

pub struct TaskManager {
    num_app: usize,
    inner: Mutex<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: [TaskControlBlock; MAX_APP_NUM],
    current_task: [usize; CPU_NUM],
    cpu_free: usize,
}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            task_cx: TaskContext::zero_init(),
            task_status: TaskStatus::UnInit,
        }; MAX_APP_NUM];
        for (i, task) in tasks.iter_mut().enumerate() {
            task.task_cx = TaskContext::goto_restore(init_app_cx(i));
            task.task_status = TaskStatus::Ready;
        }
        TaskManager {
            num_app,
            inner: unsafe {
                Mutex::new(TaskManagerInner {
                    tasks,
                    current_task: [num_app - 1; CPU_NUM],
                    cpu_free: 0,
                })
            },
        }
    };
}

impl TaskManager {
    fn find_next_task(&self, inner: &TaskManagerInner) -> Option<usize> {
        let current = inner.current_task[get_cpu_id()];
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

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
        let task0 = &mut inner.tasks[cpu_id];
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
