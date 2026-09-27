use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchTime};

pub fn dispatch_after_on_main_queue(delay_in_nanoseconds: i64, work: impl Fn() + 'static) {
    let block = RcBlock::new(work);
    unsafe {
        DispatchQueue::exec_after_with_block(
            DispatchTime::NOW.time(delay_in_nanoseconds),
            DispatchQueue::main(),
            &*block as *const _ as *mut _,
        );
    }
}

pub(crate) const NSEC_PER_SEC: u64 = 1000000000;
