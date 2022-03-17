pub mod block;
mod ide;

pub use block::BLOCK_DEVICE;
pub use ide::{MAX_PAGES, ide_read, ide_write, ide_test};
