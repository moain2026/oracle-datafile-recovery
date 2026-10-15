#![no_main]
use odr_core::charset::{Charset, decode};
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    for id in [1u16, 31, 51, 178, 560, 871, 873, 2000] {
        if let Some(cs) = Charset::from_oracle_id(id) {
            let _ = decode(data, cs);
        }
    }
});
