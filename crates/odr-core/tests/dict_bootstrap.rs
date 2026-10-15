//! `bootstrap$` → catalog, end to end (Lead L-6).
//!
//! Golden vector: the last 162 bytes of `bootstrap$` block (file 1 block 378,
//! 10g, little-endian), copied from a published BBED `dump` (cnblogs,
//! tianlesoftware 2011). They hold two real rows and the tail:
//! `LINE#=0 OBJ#=0 'CREATE ROLLBACK SEGMENT SYSTEM … OBJNO 0 EXTENTS (FILE 1 BLOCK 9))'`
//! and the version marker `LINE#=-1 OBJ#=-1 '8.0.0.0.0'`.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::similar_names
)]

use odr_core::ByteOrder;
use odr_core::charset::Charset;
use odr_core::dict::bootstrap::{Catalog, ObjectKind, RowIssue, Storage};
use odr_core::row::{DataBlock, RowPiece, flag};
use odr_core::types::OraNumber;
use proptest::prelude::*;

const GOLDEN_TAIL: &str = "2c010301 80018081 4352454154452052 4f4c4c42 41434b20 5345474d 454e5420 \
    53595354 454d2053 544f5241 47452028 2020494e49544941 4c203131 324b204e \
    45585420 31303234 4b204d49 4e455854 454e54532031204d 41584558 54454e54 \
    53203332 37363520 4f424a4e 4f203020 455854454e545320 2846494c 45203120 \
    424c4f43 4b203929 292c0103033e6466 033e6466 09382e30 2e302e30 2e300106 6101";

fn hex(s: &str) -> Vec<u8> {
    let h: String = s.chars().filter(char::is_ascii_hexdigit).collect();
    (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
        .collect()
}

/// A block whose heap ends with the golden bytes; the two rows sit at their
/// published offsets (8030 and 8167). Header fields are rebuilt around them.
fn golden_block() -> Vec<u8> {
    let bs = 8192;
    let tail = hex(GOLDEN_TAIL);
    assert_eq!(tail.len(), 162, "dump covers offsets 8030..8191");
    let mut b = vec![0u8; bs];
    b[bs - tail.len()..].copy_from_slice(&tail);
    b[0] = 0x06;
    b[1] = 0xA2;
    b[0x14] = 1;
    b[0x18..0x1C].copy_from_slice(&56u32.to_le_bytes()); // bootstrap$ objd (10g)
    b[0x24..0x26].copy_from_slice(&2u16.to_le_bytes()); // itc = 2
    let kdbh = 0x2C + 48 + 8; // ASSM
    let nrow = 2u16;
    let fsbo = 14 + 4 + 2 * nrow;
    let rows = [8030usize, 8167];
    b[kdbh + 1] = 1;
    b[kdbh + 2..kdbh + 4].copy_from_slice(&nrow.to_le_bytes());
    b[kdbh + 4..kdbh + 6].copy_from_slice(&0xFFFFu16.to_le_bytes());
    b[kdbh + 6..kdbh + 8].copy_from_slice(&fsbo.to_le_bytes());
    let fseo = (rows[0] - kdbh) as u16;
    b[kdbh + 8..kdbh + 10].copy_from_slice(&fseo.to_le_bytes());
    b[kdbh + 10..kdbh + 12].copy_from_slice(&(fseo - fsbo).to_le_bytes());
    b[kdbh + 12..kdbh + 14].copy_from_slice(&(fseo - fsbo).to_le_bytes());
    b[kdbh + 16..kdbh + 18].copy_from_slice(&nrow.to_le_bytes());
    for (i, r) in rows.iter().enumerate() {
        b[kdbh + 18 + 2 * i..kdbh + 20 + 2 * i].copy_from_slice(&((r - kdbh) as u16).to_le_bytes());
    }
    b
}

#[test]
fn golden_bootstrap_block_yields_catalog() {
    let b = golden_block();
    let db = DataBlock::parse(&b, ByteOrder::Little).unwrap();
    let rows: Vec<RowPiece<'_>> = db.rows().map(|(_, r)| r.unwrap()).collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].len, 137,
        "row 0 ends where row 1 begins (8167 - 8030)"
    );
    assert_eq!(
        OraNumber::decode(rows[1].columns[0].unwrap())
            .unwrap()
            .to_i64(),
        Some(-1)
    );

    let cat = Catalog::from_rows(rows, Charset::We8MsWin1252);
    assert!(cat.issues.is_empty(), "{:?}", cat.issues);
    assert_eq!(cat.version.as_deref(), Some("8.0.0.0.0"));
    let rbs = &cat.objects[&0];
    assert_eq!(
        (rbs.kind, rbs.name.as_str(), rbs.line),
        (ObjectKind::RollbackSegment, "SYSTEM", 0)
    );
    assert_eq!(rbs.storage, Storage::Segment { file: 1, block: 9 });
}

// --- synthetic rows built from published SQL_TEXT --------------------------

fn num(v: i64) -> Vec<u8> {
    // Minimal positive/negative integer encoder (independent of the decoder).
    if v == 0 {
        return vec![0x80];
    }
    let neg = v < 0;
    let mut n = v.unsigned_abs();
    let mut d = Vec::new();
    while n > 0 {
        d.push((n % 100) as u8);
        n /= 100;
    }
    d.reverse();
    let exp = d.len() as u8 - 1;
    while d.last() == Some(&0) {
        d.pop();
    }
    if neg {
        let mut out = vec![!(0xC1 + exp)];
        out.extend(d.iter().map(|x| 101 - x));
        if out.len() < 21 {
            out.push(102);
        }
        out
    } else {
        let mut out = vec![0xC1 + exp];
        out.extend(d.iter().map(|x| x + 1));
        out
    }
}

struct Owned(Vec<Option<Vec<u8>>>, u8);
impl Owned {
    fn row(line: i64, obj: i64, sql: &str) -> Self {
        Self(
            vec![
                Some(num(line)),
                Some(num(obj)),
                Some(sql.as_bytes().to_vec()),
            ],
            flag::H | flag::F | flag::L,
        )
    }
    fn view(&self) -> RowPiece<'_> {
        RowPiece {
            slot: 0,
            flag: self.1,
            lock: 0,
            columns: self.0.iter().map(|c| c.as_deref()).collect(),
            next: None,
            head: None,
            dscn: None,
            cluster: None,
            len: 0,
        }
    }
}

const OBJ: &str = r#"CREATE TABLE OBJ$("OBJ#" NUMBER NOT NULL,"DATAOBJ#" NUMBER) PCTFREE 10 PCTUSED 40 INITRANS 1 MAXTRANS 255 STORAGE (  INITIAL 16K NEXT 1024K MINEXTENTS 1 MAXEXTENTS 2147483645 PCTINCREASE 0 OBJNO 18 EXTENTS (FILE 1 BLOCK 121))"#;
const C_OBJ: &str = r#"CREATE CLUSTER C_OBJ#("OBJ#" NUMBER) PCTFREE 5 PCTUSED 40 INITRANS 2 MAXTRANS 255 STORAGE (  INITIAL 136K NEXT 200K MINEXTENTS 1 MAXEXTENTS 2147483645 PCTINCREASE 0 OBJNO 2 EXTENTS (FILE 1 BLOCK 144)) SIZE 800"#;
const TAB: &str = r#"CREATE TABLE TAB$("OBJ#" NUMBER NOT NULL,"CLUCOLS" NUMBER) STORAGE (  OBJNO 4 TABNO 1) CLUSTER C_OBJ#(OBJ#)"#;

#[test]
fn catalog_resolves_core_tables_and_cluster_membership() {
    let rows = [
        Owned::row(18, 18, OBJ),
        Owned::row(2, 2, C_OBJ),
        Owned::row(4, 4, TAB),
    ];
    let cat = Catalog::from_rows(rows.iter().map(Owned::view), Charset::Al32Utf8);
    assert!(cat.issues.is_empty() && cat.conflicts.is_empty());
    assert_eq!(
        cat.by_name("OBJ$").unwrap().storage,
        Storage::Segment {
            file: 1,
            block: 121
        }
    );
    let Storage::Clustered { cluster, tabno, .. } = &cat.by_name("TAB$").unwrap().storage else {
        unreachable!()
    };
    assert_eq!((cluster.as_str(), *tabno), ("C_OBJ#", 1));
    assert_eq!(
        cat.by_name(cluster).unwrap().storage,
        Storage::Segment {
            file: 1,
            block: 144
        }
    );
}

#[test]
fn inconsistent_rows_are_reported_not_used() {
    let mut deleted = Owned::row(9, 9, OBJ);
    deleted.1 |= flag::D;
    let mut bad_text = Owned::row(7, 7, "x");
    bad_text.0[2] = Some(vec![0xFF, 0xFE]); // invalid in AL32UTF8
    let rows = [
        Owned::row(18, 99, OBJ),           // OBJ# column disagrees with OBJNO
        Owned::row(3, 3, "CREATE VIEW V"), // not a bootstrap statement
        bad_text,
        deleted, // deleted rows are skipped silently
        Owned::row(2, 2, C_OBJ),
        Owned::row(2, 2, &C_OBJ.replace("BLOCK 144", "BLOCK 145")), // conflicting duplicate
    ];
    let cat = Catalog::from_rows(rows.iter().map(Owned::view), Charset::Al32Utf8);
    assert!(cat.objects.is_empty(), "{:?}", cat.objects);
    assert_eq!(cat.conflicts, vec![2]);
    assert!(matches!(
        cat.issues[0],
        (
            Some(18),
            RowIssue::ObjMismatch {
                column: 99,
                text: 18
            }
        )
    ));
    assert!(matches!(cat.issues[1], (Some(3), RowIssue::Unparsed(_))));
    assert!(matches!(cat.issues[2], (Some(7), RowIssue::BadText { .. })));
    assert_eq!(cat.issues.len(), 3);
}

#[test]
fn to_i64_is_exact() {
    for v in [
        0i64,
        1,
        -1,
        99,
        100,
        101,
        -100,
        4_194_312,
        i64::from(u32::MAX),
        1_000_000_000_000,
    ] {
        assert_eq!(OraNumber::decode(&num(v)).unwrap().to_i64(), Some(v), "{v}");
    }
    // 1.5 and 0.01 are not integers; nothing is rounded.
    assert_eq!(
        OraNumber::decode(&[0xC1, 0x02, 0x33]).unwrap().to_i64(),
        None
    );
    assert_eq!(OraNumber::decode(&[0xC0, 0x02]).unwrap().to_i64(), None);
    // 10^38 does not fit i64.
    assert_eq!(OraNumber::decode(&[0xD4, 0x02]).unwrap().to_i64(), None);
}

proptest! {
    #[test]
    fn to_i64_roundtrip(v in any::<i64>().prop_filter("min", |v| *v != i64::MIN)) {
        prop_assert_eq!(OraNumber::decode(&num(v)).unwrap().to_i64(), Some(v));
    }

    /// Arbitrary SQL_TEXT never panics and never yields an object whose
    /// OBJ# differs from the row's OBJ# column.
    #[test]
    fn catalog_total(text in ".{0,300}", obj in 0i64..100) {
        let r = Owned::row(obj, obj, &text);
        let cat = Catalog::from_rows([r.view()], Charset::Al32Utf8);
        for (k, o) in &cat.objects {
            prop_assert_eq!(i64::from(*k), obj);
            prop_assert_eq!(i64::from(o.obj), obj);
        }
    }
}
