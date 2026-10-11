//! `cargo run -p recached-mobile --features cli --bin uniffi-bindgen -- generate
//! --library <lib> --language kotlin|swift --out-dir <dir>`

fn main() {
    uniffi::uniffi_bindgen_main()
}
