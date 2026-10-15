#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    for o in [odr_core::ByteOrder::Little, odr_core::ByteOrder::Big] {
        if let Ok(db) = odr_core::row::DataBlock::parse(data, o) {
            for r in db.rows() {
                let _ = r;
            }
        }
    }
});
