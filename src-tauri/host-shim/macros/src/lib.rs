//! `#[tauri::command]` under the host shim: commands are plain functions the native UI calls
//! directly, so the attribute leaves them untouched.
use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
