# خطة المدير (Lead) — "صحة البيانات" + المراجعة
الفرع: `lead/<feature>` ← PR إلى `main`. المراجعة: يراجع Lead كل PR لـ Astra خلال نفس الجلسة؛ PRs الـ Lead يراجعها المالك أو Astra.

| # | المهمة | المتطلب | الحالة |
|---|---|---|---|
| L-1 | `row/ktb` (ktbbh + ITL + `data_object_id`) · `row/kdb` (kdbh/kdbt/kdbr باختيار الإزاحة بالتحقق البنيوي) · `row/piece` (أطوال، NULL، nrid، محذوف) | REQ-003.2/.4 | ✅ PR #2 |
| L-2 | الترميز: AR8MSWIN1256، AL32UTF8، UTF8/CESU-8، WE8MSWIN1252، WE8ISO8859P1، AR8ISO8859P6، AL16UTF16 — جدول اختبار لكل بايت 0x80–0xFF | REQ-002.4 | ⬜ |
| L-3 | الأنواع: TIMESTAMP(0–9)، TZ/LTZ، INTERVAL YM/DS، BINARY_FLOAT/DOUBLE، RAW، ROWID، CHAR | REQ-003.1 | ⬜ |
| L-4 | تجميع الصف عبر القطع (chained/migrated) + `Value` + `Provenance` | REQ-003.2 | ⬜ |
| L-5 | تأكيد كل `lab-verify` على الملفات الذهبية لـ Astra | ADR-0004 | ⬜ ينتظر L3 |
| L-6 | القاموس: bootstrap$ ← obj$/tab$/col$/user$/seg$ | REQ-002 | ⬜ |
| L-7 | الاستدلال بدون قاموس (الميزة الفارقة) | M4 | ⬜ |
