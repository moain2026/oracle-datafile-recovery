#![no_main]
//! `bootstrap$` catalog builder on arbitrary SQL_TEXT bytes: never panics and
//! never yields an object whose OBJ# differs from the row's OBJ# column.
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    use odr_core::charset::Charset;
    use odr_core::dict::bootstrap::Catalog;
    use odr_core::row::{RowPiece, flag};
    let obj: &[u8] = &[0xC1, 0x05]; // 4
    let row = RowPiece {
        slot: 0,
        flag: flag::H | flag::F | flag::L,
        lock: 0,
        columns: vec![Some(obj), Some(obj), Some(data)],
        next: None,
        head: None,
        dscn: None,
        cluster: None,
        len: 0,
    };
    let cat = Catalog::from_rows([row], Charset::Al32Utf8);
    assert!(cat.objects.keys().all(|k| *k == 4));
});
