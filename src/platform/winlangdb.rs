use std::io;
use sys;
use winapi;
use winapi::ctypes::c_char;    
use winrust::*;

pub struct LanguageData {
    pub tag: String,
    pub name: String,
    pub english_name: String,
    pub localised_name: String,
    pub script_name: String
}

pub fn remove_inputs_for_all_languages_internal() -> Result<(), io::Error> {
    let ret = unsafe { sys::winlangdb::RemoveInputsForAllLanguagesInternal() };

    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

pub fn ensure_language_profile_exists() -> Result<(), io::Error> {
    let ret = unsafe { sys::winlangdb::EnsureLanguageProfileExists() };

    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

pub fn get_language_names(tag: &str) -> Option<LanguageData> {
    let mut a = [0u16; 256];
    let mut b = [0u16; 256];
    let mut c = [0u16; 256];
    let mut d = [0u16; 256];

    let ret = unsafe {
        sys::winlangdb::GetLanguageNames(
            to_wide_string(tag).as_ptr(),
            a.as_mut_ptr(),
            b.as_mut_ptr(),
            c.as_mut_ptr(),
            d.as_mut_ptr()
        )
    };

    if ret < 0 {
        println!("{:?}", io::Error::last_os_error());
        return None;
    }

    Some(LanguageData {
        tag: tag.to_owned(),
        name: from_wide_string(&a).unwrap(),
        english_name: from_wide_string(&b).unwrap(),
        localised_name: from_wide_string(&c).unwrap(),
        script_name: from_wide_string(&d).unwrap()
    })
}

pub fn set_user_languages(tags: &[String]) -> Result<(), io::Error> {
    let handle = HString::from(tags.join(";"));
    let ret = unsafe { sys::winlangdb::SetUserLanguages(';' as c_char, *handle) };
    
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

pub fn transform_input_methods(methods: InputList, tag: &str) -> InputList {
    let hmethods = HString::from(methods.0);
    let htag = HString::from(tag);
    let out = unsafe {
        let mut out = HString::null();
        sys::winlangdb::TransformInputMethodsForLanguage(*hmethods, *htag, &mut *out);
        out
    };
    InputList(String::from(out))
}

pub fn default_input_method(tag: &str) -> InputList {
    let htag = HString::from(tag);
    let out = unsafe {
        let mut out = HString::null();
        sys::winlangdb::GetDefaultInputMethodForLanguage(*htag, &mut *out);
        out
    };
    InputList(String::from(out))
}