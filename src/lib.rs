extern crate winapi;
extern crate winreg;


use winapi::shared::minwindef::{DWORD, LPARAM};
use winapi::shared::ntdef::*;
use winreg::RegKey;
use winreg::enums::*;
use winreg::types::{ToRegValue, FromRegValue};
use std::ptr::null_mut;
use std::io;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};

use winrust::*;
use platform::*;
use types::*;

mod types;
mod winrust;
mod platform;

pub mod input {
    use ::*;

    pub fn install_layout(inputs: InputList) -> Result<(), io::Error> {
        let winput = to_wide_string(&inputs.0);

        let ret = unsafe { sys::input::InstallLayoutOrTip(winput.as_ptr(), 0) };
        if ret < 0 {
            return Err(io::Error::last_os_error())
        }

        Ok(())
    }
}

fn set_user_languages(tags: &[String]) -> Result<(), String> {
    let valid_tags: Vec<String> =
        tags.iter()
        .flat_map(|t| winlangdb::get_language_names(t))
        .map(|t| t.tag)
        .collect();

    println!("{:?}", valid_tags);

    winlangdb::set_user_languages(&valid_tags)
        .or_else(|_| Err("Failed enabling languages".to_owned()))?;
    winlangdb::remove_inputs_for_all_languages_internal()
        .or_else(|_| Err("Remove inputs failed".to_owned()))?;

    let results: Vec<Result<(), String>> = valid_tags.iter().map(|tag| {
        let lcid = match bcp47langs::lcid_from_bcp47(tag) {
            Some(v) => v,
            None => return Err("Failed to enable tag".to_owned())
        };

        let mut inputs = winlangdb::default_input_method(tag);
        inputs = winlangdb::transform_input_methods(inputs, tag);
        println!("{} {}", tag, inputs.0);
        input::install_layout(inputs).or_else(|_| Err("failed to enable layout".to_owned()))
    }).collect();

    let errors: Vec<&Result<(), String>> = results.iter().filter(|x| x.is_err()).collect();

    if errors.len() > 0 {
        return Err("There were some errors while enabling layouts, and were ignored.".to_owned())
    }

    Ok(())
}

pub fn query_language(tag: &str) -> String {
    let id = winnls::resolve_locale_name(tag)
        .unwrap_or(tag.to_owned());

    match winlangdb::get_language_names(&id) {
        None => format!("{}: Unsupported tag.", &id),
        Some(data) => {
            format!("\
Tag: {}
Name: {}
English Name: {}
Native Name: {}
Script: {}", id, data.name, data.english_name, data.localised_name, data.script_name)
        }
    }
}

#[test]
fn test_lcid_from_bcp47() {
    assert_eq!(bcp47langs::lcid_from_bcp47("en-AU"), Some(0x0c09), "en-AU");
    assert_eq!(bcp47langs::lcid_from_bcp47("vro-Latn"), Some(0x2000), "vro-Latn");
    assert_eq!(bcp47langs::lcid_from_bcp47("sjd-Cyrl"), Some(0x1000), "sjd-Cyrl");
}

#[test]
fn test_default_input() {
    assert_eq!(winlangdb::default_input_method("en-AU"), InputList("0C09:00000409".to_owned()))
}

#[test]
fn test_transform_input() {
    assert_eq!(
        winlangdb::transform_input_methods(InputList("0C09:00000409".to_owned()), "en-AU"),
        InputList("0C09:00000409".to_owned())
    );
}



fn user_profile() -> RegKey {
    RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Control Panel\\International\\User Profile", KEY_READ | KEY_WRITE)
            .unwrap()
}

pub fn enabled_languages() -> Result<Vec<String>, io::Error> {
    winlangdb::ensure_language_profile_exists()?;
    bcp47langs::get_user_languages()
}

// TODO: reimplement support for adding native language name, optionally
pub fn enable_language(tag: &str) -> Result<(), io::Error> {
    let mut langs = enabled_languages()?;
    
    let lang = tag.to_owned();

    if langs.contains(&lang) {
        return Ok(());
    }
    
    langs.push(lang);

    set_user_languages(&langs).unwrap();
    Ok(())
    //    .or_else(|_| Err("Error while setting languages.".to_owned()))
}

unsafe fn lpwstr_to_string(lpw: LPWSTR) -> String {
    let mut buf: Vec<WCHAR> = vec![];
    let mut i = 0isize;

    while *lpw.offset(i) != 0 {
        buf.push(*lpw.offset(i));
        i += 1
    }

    return String::from_utf16_lossy(&buf);
}

pub fn system_locales() -> Vec<String> {
    unsafe extern "system" fn callback(locale: LPWSTR, _: DWORD, l_param: LPARAM) -> i32 {
        let s = lpwstr_to_string(locale);

        let vec = l_param as *mut Vec<String>;
        (*vec).push(s);
        1
    }

    let raw_vec = Box::into_raw(Box::new(vec![]));

    unsafe {
        winapi::um::winnls::EnumSystemLocalesEx(Some(callback), 0, raw_vec as LPARAM, null_mut());
        *Box::from_raw(raw_vec)
    }
}

fn locale_name_to_lcid(locale_name: &str) -> Result<u32, io::Error> {
    let loc_name: Vec<u16> = to_wide_string(locale_name);
    
    unsafe {
        let ret = winapi::um::winnls::LocaleNameToLCID(loc_name.as_ptr(), 0);

        match ret {
            0 => Err(io::Error::last_os_error()),
            _ => Ok(ret)
        }
    }
}

pub struct LanguageRegKey {
    id: String,
    regkey: RegKey
}

impl LanguageRegKey {
    fn next_transient_lang_id() -> u32 {
        let regkey = user_profile();

        regkey.enum_keys()
            .map(|x| regkey.open_subkey(x.unwrap()))
            .fold(0x2000u32, |acc, x| {
                if let Ok(lang_id) = x.unwrap().get_value::<u32, _>("TransientLangId") {
                    if lang_id >= acc {
                        lang_id + 0x400
                    } else {
                        acc
                    }
                } else {
                    acc
                }
            })
    }

    fn next_layout_order(&self) -> u32 {
        self.regkey.enum_values()
            .fold(1u32, |acc, x| {
                let (k, v) = x.unwrap();

                if k.contains(":") {
                    if let Ok(vv) = u32::from_reg_value(&v) {
                        if vv >= acc {
                            return acc + 1;
                        }
                    }
                }

                acc
            })
    }

    fn set_language_name(&mut self, name: &str) {
        self.regkey.set_value("CachedLanguageName", &name).unwrap();
    }

    // fn language_name(&self) -> Option<String> {
    //     if let Ok(value) = self.regkey.get_value("CachedLanguageName") {
    //         Some(value)
    //     } else {
    //         None
    //     }
    // }

    fn set_transient_lang_id(&mut self, id: u32) {
        self.regkey.set_value("TransientLangId", &id).unwrap();
    }

    fn transient_lang_id(&self) -> Option<u32> {
        if let Ok(value) = self.regkey.get_value("TransientLangId") {
            Some(value)
        } else {
            None
        }
    }

    fn add_keyboard(&mut self, keyboard: KeyboardRegKey) {
        let lcid = if let Some(v) = self.transient_lang_id() {
            v
        } else {
            locale_name_to_lcid(self.id.split(r"\").last().unwrap()).unwrap()
        } as u16;
        let kbd_id = keyboard.name();

        // Add keyboard id to reg
        self.regkey.set_value(format!("{:04X}:{}", &lcid, &kbd_id.to_uppercase()), &self.next_layout_order()).unwrap();

        // Get sub id
        let sub_id = format!("{:08x}", next_substitute_id(lcid));
        
        // Create substitute entry
        kbd_layout_sub_regkey(true).set_value(&sub_id, &kbd_id).unwrap();
        kbd_layout_sub_regkey(false).set_value(&sub_id, &kbd_id).unwrap();

        // Create preload entry
        kbd_layout_preload_regkey(true).set_value(format!("{}", next_preload_id(true)), &sub_id).unwrap();
        kbd_layout_preload_regkey(false).set_value(format!("{}", next_preload_id(false)), &sub_id).unwrap();
    }

    pub fn create(alpha_3_code: &str, native_name: &str) -> LanguageRegKey {
        if let Some(lang_regkey) = LanguageRegKey::find_by_alpha_3_code(&alpha_3_code) {
            return lang_regkey;
        }

        let mut regkey = LanguageRegKey {
            id: alpha_3_code.to_owned(),
            regkey: RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey_with_flags(format!(r"Control Panel\International\User Profile\{}", &alpha_3_code), KEY_READ | KEY_WRITE)
                .unwrap()
        };

        if !system_locales().contains(&alpha_3_code.to_owned()) {
            regkey.set_language_name(native_name);
            regkey.set_transient_lang_id(LanguageRegKey::next_transient_lang_id());
        }

        regkey
    }

    fn find_by_alpha_3_code(alpha_3_code: &str) -> Option<LanguageRegKey> {
        let maybe_regkey = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(format!(r"Control Panel\International\User Profile\{}", &alpha_3_code), KEY_READ | KEY_WRITE);

        if let Ok(regkey) = maybe_regkey {
            Some(LanguageRegKey { id: alpha_3_code.to_owned(), regkey: regkey })
        } else {
            None
        }
    }

}

fn base_regkey(is_all_users: bool) -> RegKey {
    match is_all_users {
        true => {
            RegKey::predef(HKEY_USERS)
                .open_subkey_with_flags(r".DEFAULT", KEY_READ | KEY_WRITE)
                .unwrap()
        },
        false => RegKey::predef(HKEY_CURRENT_USER)
    }
}

fn kbd_layout_sub_regkey(is_all_users: bool) -> RegKey {
    base_regkey(is_all_users)
        .open_subkey_with_flags(r"Keyboard Layout\Substitutes", KEY_READ | KEY_WRITE)
        .unwrap()
}

fn kbd_layout_preload_regkey(is_all_users: bool) -> RegKey {
    base_regkey(is_all_users)
        .open_subkey_with_flags(r"Keyboard Layout\Preload", KEY_READ | KEY_WRITE)
        .unwrap()
}

/// Substitute IDs begin with 0000, then increment to d001, and continue incrementing dXXX.
fn next_substitute_id(suffix: u16) -> u32 {
    let prefix: u16 = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"Keyboard Layout\Substitutes", KEY_READ)
        .unwrap()
        .enum_values()
        .fold(0u16, |acc, x| {
            let (k, _) = x.unwrap();

            if let Ok(kk) = u32::from_str_radix(&k, 16) {
                if (kk as u16) == suffix {
                    // Move high bits down
                    let v = (kk >> 16) as u16;

                    if v >= acc {
                        return if v == 0 {
                            0xd001
                        } else {
                            v + 1
                        }
                    }
                }

                acc
            } else {
                acc
            }
        });

    ((prefix as u32) << 16) + (suffix as u32)
}

fn next_preload_id(is_all_users: bool) -> u32 {
    base_regkey(is_all_users)
        .open_subkey_with_flags(r"Keyboard Layout\Preload", KEY_READ)
        .unwrap()
        .enum_values()
        .fold(1u32, |acc, x| {
            let (k, _) = x.unwrap();

            if let Ok(v) = u32::from_str_radix(&k, 10) {
                if v >= acc {
                    v + 1
                } else {
                    acc
                }
            } else {
                acc
            }
        })
}

#[test]
fn test_sub_id() {
    println!("sub_id: {:08x}", next_substitute_id(0xabcd));
    println!("sub_id: {:08x}", next_substitute_id(0x0c09));
}

#[test]
fn test_it_doth_work() {
    let v = LanguageRegKey::next_transient_lang_id();

    println!("Transient id: {:04x}", v);
}

pub fn install_keyboard(
    language_name: &str,
    product_code: &str,
    layout_file: &str,
    layout_name: &str,
    language_code: &str
) {
    let mut lang = LanguageRegKey::create(language_code, language_name);
    enable_language(language_code);

    let kbd = KeyboardRegKey::create(language_code, language_name, product_code, layout_file, layout_name);
    lang.add_keyboard(kbd);
}
