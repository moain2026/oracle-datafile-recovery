# سجل `lab-verify` — كل ثابت ثنائي لم يُؤكَّد بعد على ملفات أوراكل حقيقية
كل صف هنا يحتاج اختباراً على ملف ذهبي (Astra L4). عند التأكيد: احذف الوسم من الكود وانقل الصف إلى "مؤكَّد".

| المعرّف | الموقع في الكود | الادعاء | المصدر | الحالة |
|---|---|---|---|---|
| LV-001 | `block.rs` | أنواع الكتل 0x0E/0x10/0x1D/0x1E/0x20–0x23 | Dyke [W2] | ⬜ |
| LV-002 | `block.rs` | tailchk = (scn_base&0xFFFF)<<16 · type<<8 · seq | [W3]، RICO2 README | ⬜ |
| LV-003 | `row/ktb.rs` | ktbbh: typ@0x14، objd@0x18، csc@0x1C، itc@0x24، flg@0x26، fsl@0x27، fnx@0x28، ITL@0x2C × 24 بايت | BBED struct dumps (منشورة) | ⬜ |
| LV-004 | `row/ktb.rs` | تخطيط مدخل ITL: xid(8)، uba(8)، flag/lck(2)، fsc(2)، scn base(4) | Lewis [A11] | ⬜ |
| LV-005 | `row/kdb.rs` | kdbh عند `0x2C + 24·itc` (MSSM) أو `+8` (ASSM)؛ نختار بالتحقق البنيوي | BBED dumps | ⬜ |
| LV-006 | `row/kdb.rs` | `fsbo = 14 + 4·ntab + 2·nrow` (نهاية دليل الصفوف) | BBED/block dumps | ⬜ |
| LV-007 | `row/kdb.rs` | إزاحات kdbr نسبية لبداية kdbh | BBED `*kdbr[n]` | ⬜ |
| LV-008 | `row/piece.rs` | nrid (dba 4 + slot 2، بترتيب الملف) موجود iff لم يُرفع العلم L | Lewis [W6] | ⬜ |
| LV-009 | `row/piece.rs` | الطول الطويل `0xFE` + u16 **big-endian** على كل المنصات | oradul [G1] | ⬜ |
| LV-010 | `row/piece.rs` | قطعة الوجهة لصف مهاجر قد تحمل hrid — غير مدعوم بعد | Lewis [W6] | ⬜ |
