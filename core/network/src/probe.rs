use std::ffi::CString;

#[cfg(enable_probe)]
#[macro_use]
#[no_link]
extern crate probe;

#[cfg(not(enable_probe))]
#[macro_export]
macro_rules! probe {
    ($provider:ident, $name:ident) => {};
    ($provider:ident, $name:ident, $($arg:expr),*) => {
        $(let _ = $arg;)*
    };
}

pub fn cstring(s: &str) -> CString {
    match CString::new(s) {
        Ok(s) => s,
        Err(e) => {
            log::error!("network probe: nul string {}", s);

            let nul_pos = e.nul_position();
            let (truncated, _) = s.split_at(nul_pos);

            CString::new(truncated).unwrap_or_else(|_| CString::new("").expect("impossible"))
        }
    }
}
