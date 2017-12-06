use libc::{c_int,c_char};
use std::ffi::{CString,CStr};
use std::fs::File;
use std::ops::Deref;
use std::fmt;
use std::ptr;
use std::option::Option;
use std::os::unix::io::FromRawFd;
use std::marker::PhantomData;
use std::sync::{Once, ONCE_INIT};


#[link(name = "ue-apt-c", kind ="static")]
#[link(name = "apt-pkg")]
extern {
    fn apt_c_get_lock(file: *const c_char) -> c_int;
    fn apt_c_pop_last_error_owned() -> *mut c_char;
    fn apt_c_free_string(str_ptr: *const c_char);
    fn apt_c_init_system();
    fn apt_c_config_get_owned_str(key: *const c_char, def: *const c_char) -> *mut c_char;
}

static INIT: Once = ONCE_INIT;

pub struct AptCString {
    string: *mut c_char
}

impl fmt::Display for AptCString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cast: &CStr = self;
        return cast.to_string_lossy().fmt(f);
    }
}

impl fmt::Debug for AptCString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cast: &CStr = self;
        return cast.to_string_lossy().fmt(f);
    }
}

impl Drop for AptCString {
    fn drop(&mut self) {
        unsafe {
            apt_c_free_string(self.string);
        }
    }
}

impl Deref for AptCString {
    type Target = CStr;

    fn deref(&self) -> &CStr {
        unsafe {
            return CStr::from_ptr(self.string)
        }
    }
}

pub struct AptFileLock<'a>(pub &'a str);
pub struct AptFileLockGuard(File);
impl <'a> AptFileLock<'a> {
    pub fn lock(self) -> Result<AptFileLockGuard,&'static str> {
        if let Ok(file_name) = CString::new(self.0) {
            unsafe {
                let fd = apt_c_get_lock(file_name.as_ptr());
                if fd < 0 {
                    Err("Could not acquire lock (msg TODO)")
                } else {
                    Ok(AptFileLockGuard(File::from_raw_fd(fd)))
                    
                }
            }
        } else {
            Err("Invalid file name")
        }
    }
}

pub fn last_error_consuming() -> Option<AptCString> {
    unsafe {
        let err = apt_c_pop_last_error_owned();
        if err.is_null() {
            None
        } else {
            Some(AptCString{string: err})
        }
    }
}

pub struct Config{phantom: PhantomData<()>}

impl Config {

    pub fn get() -> Config {
        INIT.call_once(|| {
           unsafe { apt_c_init_system() };
        });
        return Config{phantom: PhantomData};
    }
    pub fn find_string<'a, T: Into<Option<&'a str>>>(&self, key: & 'a str, default: T) -> Result<AptCString,&'static str> {

        let def = default.into().and_then(|x| CString::new(x).ok())
            .map(|x| x.as_ptr())
            .unwrap_or(ptr::null());
        if let Ok(key_str) = CString::new(key) {
            unsafe {
                let val = apt_c_config_get_owned_str(key_str.as_ptr(), def);
                return Ok(AptCString{string: val});
            }
        } else {
            return Err("Invalid key");
        }
    }
}