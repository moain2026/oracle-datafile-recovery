#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = odr_core::types::OraNumber::decode(data).map(|n| n.to_string());
    let _ = odr_core::types::OraDate::decode(data).map(|d| d.to_string());
});
