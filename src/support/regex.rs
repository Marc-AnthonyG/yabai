const _: () = assert!(std::mem::size_of::<libc::regex_t>() == 32);

pub struct PosixRegex {
    regex: Box<libc::regex_t>,
}

impl PosixRegex {
    pub fn compile(pattern: &std::ffi::CStr) -> Option<PosixRegex> {
        let mut regex: Box<libc::regex_t> = Box::new(unsafe { std::mem::zeroed() });
        let status = unsafe { libc::regcomp(regex.as_mut(), pattern.as_ptr(), libc::REG_EXTENDED) };
        if status == 0 {
            Some(PosixRegex { regex })
        } else {
            None
        }
    }

    pub fn matches(&self, subject: &std::ffi::CStr) -> bool {
        let status = unsafe {
            libc::regexec(self.regex.as_ref(), subject.as_ptr(), 0, std::ptr::null_mut(), 0)
        };
        status == 0
    }
}

impl Drop for PosixRegex {
    fn drop(&mut self) {
        unsafe { libc::regfree(self.regex.as_mut()) };
    }
}

#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RegexMatch {
    Undefined = 0,
    Yes = 1,
    No = 2,
}

pub fn regex_match(regex: Option<&PosixRegex>, subject: &std::ffi::CStr) -> RegexMatch {
    match regex {
        None => RegexMatch::Undefined,
        Some(regex) => {
            if regex.matches(subject) {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            }
        }
    }
}
