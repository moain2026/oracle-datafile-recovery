#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    for o in [odr_core::ByteOrder::Little, odr_core::ByteOrder::Big] {
        let _ = odr_core::block::classify(data, 2048, o);
        let _ = odr_core::scan::scan_image(data, 2048, o);
    }
});
