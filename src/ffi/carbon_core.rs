#[repr(C)]
#[derive(Clone, Copy)]
pub struct UnsignedWide {
    pub hi: u32,
    pub lo: u32,
}
pub type AbsoluteTime = UnsignedWide;
pub type Nanoseconds = UnsignedWide;

const _: () = assert!(core::mem::size_of::<UnsignedWide>() == 8);

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    pub fn AbsoluteToNanoseconds(absolute_time: AbsoluteTime) -> Nanoseconds;
}

#[allow(deprecated)]
pub fn read_os_timer() -> u64 {
    let result = unsafe { libc::mach_absolute_time() };
    let nanoseconds =
        unsafe { AbsoluteToNanoseconds(core::mem::transmute::<u64, AbsoluteTime>(result)) };
    unsafe { core::mem::transmute::<Nanoseconds, u64>(nanoseconds) }
}

pub fn read_os_freq() -> u64 {
    1000000000
}
