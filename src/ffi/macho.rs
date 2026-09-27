#![allow(deprecated)]

use core::ffi::{CStr, c_char, c_void};
use libc::{LC_SEGMENT_64, load_command, mach_header_64, segment_command_64};

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct symtab_command {
    pub cmd: u32,
    pub cmdsize: u32,
    pub symoff: u32,
    pub nsyms: u32,
    pub stroff: u32,
    pub strsize: u32,
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct nlist_64 {
    pub n_strx: u32,
    pub n_type: u8,
    pub n_sect: u8,
    pub n_desc: u16,
    pub n_value: u64,
}

pub const LC_SYMTAB: u32 = 0x2;
pub const SEG_LINKEDIT: &[u8] = b"__LINKEDIT";

fn segment_name_is_linkedit(segname: &[c_char; 16]) -> bool {
    let bytes: &[u8; 16] = unsafe { &*(segname as *const [c_char; 16] as *const [u8; 16]) };
    &bytes[..SEG_LINKEDIT.len()] == SEG_LINKEDIT && bytes[SEG_LINKEDIT.len()] == 0
}

unsafe fn macho_find_image_header(target_name: &CStr, slide: &mut u64) -> *const mach_header_64 {
    let image_count = unsafe { libc::_dyld_image_count() };

    for index in 0..image_count {
        let image_name = unsafe { libc::_dyld_get_image_name(index) };
        if image_name.is_null() {
            continue;
        }

        if unsafe { CStr::from_ptr(image_name) } == target_name {
            *slide = unsafe { libc::_dyld_get_image_vmaddr_slide(index) } as u64;
            return unsafe { libc::_dyld_get_image_header(index) } as *const mach_header_64;
        }
    }

    core::ptr::null()
}

unsafe fn macho_find_linkedit_segment(header: *const mach_header_64) -> *const segment_command_64 {
    let mut offset = core::mem::size_of::<mach_header_64>();

    for _ in 0..unsafe { (*header).ncmds } {
        let command = unsafe { (header as *const u8).add(offset) } as *const load_command;

        if unsafe { (*command).cmd } == LC_SEGMENT_64 {
            let segment = command as *const segment_command_64;
            if segment_name_is_linkedit(unsafe { &(*segment).segname }) {
                return segment;
            }
        }

        offset += unsafe { (*command).cmdsize } as usize;
    }

    core::ptr::null()
}

unsafe fn macho_find_symtab_command(header: *const mach_header_64) -> *const symtab_command {
    let mut offset = core::mem::size_of::<mach_header_64>();

    for _ in 0..unsafe { (*header).ncmds } {
        let command = unsafe { (header as *const u8).add(offset) } as *const load_command;

        if unsafe { (*command).cmd } == LC_SYMTAB {
            return command as *const symtab_command;
        }

        offset += unsafe { (*command).cmdsize } as usize;
    }

    core::ptr::null()
}

pub unsafe fn macho_find_symbol(target_image: &CStr, target_symbol: &CStr) -> Option<*mut c_void> {
    let mut slide: u64 = 0;
    let header = unsafe { macho_find_image_header(target_image, &mut slide) };
    if header.is_null() {
        return None;
    }

    let linkedit_segment = unsafe { macho_find_linkedit_segment(header) };
    if linkedit_segment.is_null() {
        return None;
    }

    let symtab_command = unsafe { macho_find_symtab_command(header) };
    if symtab_command.is_null() {
        return None;
    }

    let symbol_count = unsafe { (*symtab_command).nsyms };
    let linkedit_base =
        (unsafe { (*linkedit_segment).vmaddr } - unsafe { (*linkedit_segment).fileoff }) as usize;
    let symbol_str = (linkedit_base + unsafe { (*symtab_command).stroff } as usize + slide as usize)
        as *const u8;
    let symbol_sym = (linkedit_base + unsafe { (*symtab_command).symoff } as usize + slide as usize)
        as *const u8;

    for index in 0..symbol_count {
        let list = unsafe { symbol_sym.add(index as usize * core::mem::size_of::<nlist_64>()) }
            as *const nlist_64;
        let symbol_name = unsafe { symbol_str.add((*list).n_strx as usize) } as *const c_char;
        if unsafe { CStr::from_ptr(symbol_name) } == target_symbol {
            return Some((unsafe { (*list).n_value } + slide) as *mut c_void);
        }
    }

    None
}
