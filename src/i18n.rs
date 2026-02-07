use sys_locale::get_locale;

pub fn init() {
    let locale = get_locale().unwrap_or_else(|| String::from("en"));
    let lang = if locale.starts_with("ja") { "ja" } else { "en" };
    rust_i18n::set_locale(lang);
}
