fn keyboard_layouts_regkey() -> RegKey {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags("SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts", KEY_READ | KEY_WRITE)
        .unwrap()
}

fn first_available_keyboard_regkey_id(lcid: &str) -> String {
    let regkey = keyboard_layouts_regkey();
    let mut kbd_keys: Vec<u16> = regkey.enum_keys()
        .map(|x| x.unwrap())
        .filter(|x| x.starts_with(&"a") && x.ends_with(&lcid))
        .map(|x| {
            let n = u32::from_str_radix(&x, 16).unwrap_or(0u32);
             (n >> 16) as u16
        })
        .collect();
    
    kbd_keys.sort();

    if let Some(last) = kbd_keys.last() {
        format!("{:04x}{}", last + 1, lcid)
    } else {
        format!("a000{}", lcid)
    }
}

fn first_available_layout_id() -> String {
    let regkey = keyboard_layouts_regkey();
    let kbd_keys: Vec<String> = regkey.enum_keys().map(|x| x.unwrap()).collect();

    let mut layout_ids: Vec<u32> = kbd_keys.into_iter().map(|key| {
        let kbdkey = &regkey.open_subkey_with_flags(key, KEY_READ | KEY_WRITE).unwrap();
        let layout_idstr: String = kbdkey.get_value("Layout Id").unwrap_or("0".to_string());

        u32::from_str_radix(&layout_idstr, 16).unwrap_or(0u32)
    }).collect();

    layout_ids.sort();
    
    format!("{:04x}", layout_ids.last().unwrap() + 1)
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
