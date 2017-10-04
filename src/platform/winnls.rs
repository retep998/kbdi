use std::io;
use std::ffi::OsString;
use winapi;
use winrust::{from_wide_string, to_wide_string};

const MAX_LOCALE_NAME_LEN = 85;

pub fn resolve_locale_name(tag: &str) -> Option<String> {
    let mut buf = vec![0u16; MAX_LOCALE_NAME_LEN];

    let ret = unsafe {
        winapi::um::winnls::ResolveLocaleName(
            to_wide_string(tag).as_ptr(),
            buf.as_mut_ptr(),
            MAX_LOCALE_NAME_LEN
        )
    };
    
    if ret == 0 {
        let err = io::Error::last_os_error();
        println!("{:?}", err);
        panic!();
    }

    buf.truncate(ret as usize - 1);

    if buf.len() == 0 {
        return None;
    }

    Some(from_wide_string(buf).unwrap())
}