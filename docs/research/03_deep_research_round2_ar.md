# البحث العميق — الجولة الثانية (2026-10-15)

**الهدف:** كشف الثغرات التي كانت ستُنتج قيماً **خاطئة بصمت** أو تمنع وضع القاموس، من مصادر متعددة اللغات (صينية، برتغالية، يابانية، روسية، إنجليزية)، كلها تفريغات حقيقية لكتل أوراكل (`ALTER SYSTEM DUMP` / BBED). كل ادعاء مربوط بمعرّف `lab-verify` ولا يُعتبر مؤكَّداً حتى يُختبر على ملفات المختبر (Astra L4).

**مستويات الدليل:** T1 = توثيق أوراكل الرسمي · T2 = تفريغتان مستقلتان متطابقتان أو أكثر · T3 = تفريغة منشورة واحدة · T4 = استنتاج.

---

## 1. الاكتشافات الحرجة (غيّرت الكود فوراً)

| # | الاكتشاف | الأثر لو تجاهلناه | الإجراء | الدليل |
|---|---|---|---|---|
| C1 | **ROWDEPENDENCIES**: علم `kdbh` = `0x20` (`flag=--R-----`)، وكل صف فيه 6 بايت `dscn` (wrap u16 + base u32) بعد `cc` مباشرة | **انزياح كل الأعمدة 6 بايت → بيانات خاطئة بصمت** في كل جدول أُنشئ `ROWDEPENDENCIES` | `PieceFormat{row_dependencies}` من علم الكتلة؛ `RowPiece.dscn`؛ القطع المتسلسلة/المهاجرة مع dscn = `Unsupported` (التخطيط غير منشور) | T3 (aliyun 2012، BBED بالبايت + `tl: 12`) — LV-018/019 |
| C2 | **قاموس البيانات داخل clusters**: `TAB$`, `COL$`, `IND$`, `ICOL$`, `LOB$`, `COLTYPE$` داخل `C_OBJ#`؛ `USER$`/`TSQ$` داخل `C_USER#`؛ `TS$` داخل `C_TS#` | بدون دعم cluster **يستحيل** وضع القاموس (M3) | `row/cluster.rs`: صف المفتاح وصف العضو، `DataBlock::cluster_key`، `table_of`، `cluster_columns` | T2 (itpub 2020 لـ `C_USER#`، enmotech، tomcoding) — LV-020/021 |
| C3 | **الضغط Basic/OLTP**: علم `0x40` (`flag=-0------`)، `ntab=2` (جدول 0 = جدول الرموز)، حقول `9ir2` إضافية (`r0_9ir2, mec_kdbh9ir2, shcf, flag_9ir2, fcls_9ir2[], perm_9ir2[]`) قبل `kdbt`، والأعمدة مخزّنة بترتيب `perm_9ir2` | فكّها كصفوف عادية = **قيم خاطئة** | كانت تُرفض أصلاً (ثابت `fsbo` يفشل)؛ الآن رفض صريح `Unsupported("compressed data block (9ir2)")` بتشخيص واضح | T2 (CSDN/itpub 2013، commitandconquer 2025) — LV-022 |
| C4 | **Fast ADD COLUMN** (11g: `NOT NULL DEFAULT`؛ 12c: nullable `DEFAULT` + عمود مخفي `SYS_NCxxxxx$` يحمل bit-vector): الصفوف القديمة **لا تخزّن** العمود الجديد، وأوراكل يعيد القيمة الافتراضية من القاموس (`COL$.PROPERTY & 0x40000000`، القيمة في `ECOL$.BINARYDEFVAL`/`DEFAULT$`) | إخراج NULL بدل القيمة الافتراضية = **قيمة خاطئة** | **مهمة L-6**: عند غياب العمود في صف قديم وعلَم الخاصية مرفوع ← القيمة الافتراضية مع وسم المصدر؛ وفي 12c نقرأ bit-vector العمود المخفي (`SYS_OP_VECBIT`) لنميّز "NULL صريح" من "افتراضي" | T2 (StackOverflow 32847399، aravinddarla 2019، Tencent 114456101) — LV-023 |
| C5 | **الحذف لا يمسح البيانات** في الجداول العادية: يُرفع `D` فقط (`0x2C → 0x3C`)، والأعمدة باقية في الكتلة ما لم يُعَد استخدام المساحة؛ الطول المعروض `tl: 2` لأن أوراكل لا يطبع الأعمدة. **أما صف العضو المحذوف في cluster** فيبقى `cc: 0` (`tl: 4`) — لا شيء للاسترجاع | فرصة استرجاع (M7) | `parse_piece` يقرأ `cc` كما هو؛ وضع "الصفوف المحذوفة" لاحقاً يجب أن يتحقق من أن البايتات لم يُعَد استخدامها (تداخل مع صف حي ← رفض) | T2 (orasos/xifenfei 2012 بالبايت، zhizuobiao 2018) — LV-024 |

---

## 2. bootstrap$ ونقطة دخول القاموس (L-6)

- `kcvfhrdb` في كتلة 1 من الملف 1 عند الإزاحة **@96** (ub4) = rdba لرأس مقطع `bootstrap$` (عادة `0x00400208` = ملف 1 كتلة 520؛ البيانات في 521–523). المصادر: orasos (DUL 12c)، atoracle.cn (BBED map كامل)، CSDN، **tomcoding «Oracle DUL 程序设计» 2017** (وثيقة تصميم DUL مستقلة: "root dba … 保存在 1 号数据文件 … block 1"). — LV-025
- `bootstrap$(LINE#, OBJ#, SQL_TEXT)`: نصوص DDL فيها `STORAGE (OBJNO n TABNO t) CLUSTER C_OBJ#(...)` و`EXTENTS (FILE f BLOCK b)` ← عناوين رؤوس مقاطع الجداول الأساسية.
- **12c CDB:** لكل PDB قاموسه وbootstrap$ خاص به؛ rdba نسبي للـ PDB (relfile# يبدأ من 1)؛ `container$` يحمل rdba لكل con_id. — LV-026
- **خطة بديلة عند تلف bootstrap$** (من tomcoding، وتطابق نهج DUL): مسح SYSTEM كاملاً، وتجميع الكتل حسب `objd`؛ قيم `obj#`/`dataobj#` للجداول الأساسية ثابتة عبر الإصدارات (OBJ$=18، C_OBJ#=2، C_USER#=10، C_TS#=6، UNDO$=15)، وكذلك `TAB#` داخل الـ cluster (C_OBJ#: 1=TAB$، 3=IND$، 4=ICOL$، 5=COL$، 6=LOB$، 7=COLTYPE$؛ C_TS#: 2=TS$). **قيم يجب تأكيدها لكل إصدار في المختبر** — LV-027.
- **فقد SYSTEM كاملاً** ← وضع الاستدلال (L-7): الأنواع ثابتة الطول (DATE 7، TIMESTAMP 7/11/13، INTERVAL 5/11، BINARY_FLOAT 4، BINARY_DOUBLE 8) وأنماط NUMBER/CHAR هي الإشارات الأقوى؛ VARCHAR2 مقابل RAW تحتاج قراراً بشرياً (نفس استنتاج tomcoding).

### خريطة `kcvfh` (ملف 1 كتلة 1، 11g؛ T3 من atoracle.cn، لـ Astra S1) — LV-028
| الإزاحة | الحقل | | الإزاحة | الحقل |
|---|---|---|---|---|
| @20 | kccfhswv (إصدار البرنامج) | | @100 | kcvfhcrs (SCN الإنشاء) |
| @24 | kccfhcvn (compat) | | @108 | kcvfhcrt |
| @28 | kccfhdbi (DBID) | | @112/@116 | resetlogs time/SCN |
| @32 | kccfhdbn[8] (اسم القاعدة) | | @138 | kcvfhsta u16 (الحالة/fuzzy) |
| @44 | kccfhfsz (حجم الملف بالكتل) | | @140 | kcvfhcpc |
| @48 | kccfhbsz (حجم الكتلة) | | @332/@336/@338 | ts#، طول الاسم، اسم tablespace[30] |
| @52 | kccfhfno u16 | | @368 | kcvfhrfn (relative file#) |
| @54 | kccfhtyp (3 = DATA) | | @484/@488/@492 | checkpoint SCN base/wrap/time |
| @96 | **kcvfhrdb** | | | |

حجم البنية يختلف بالإصدار (10g ≈ 676، 11g ≈ 860، 12c ≈ 1112 بايت) ← **الإزاحات بعد @140 تُتحقق لكل إصدار**.

---

## 3. صيغة صفوف الـ cluster (المُنفَّذة)

```text
مفتاح  (K-H-FL-- = 0xAC):  flag lock cc | curc u16 | comc u16 | prev rid (dba4 slot2) | next rid (6) | أعمدة المفتاح
عضو    (-CH-FL-- = 0x6C):  flag lock cc | cki u8 | أعمدة العضو (بدون أعمدة المفتاح)
عضو محذوف (-CHDFL- = 0x7C): cc = 0
```

إثبات حسابي من التفريغات (كل `tl` يطابق حسابنا، والاختبار `cluster_rowdep.rs` يتحقق منه):
- مفتاح 20 (`c1 15`): 3 + 2 + 2 + 6 + 6 + (1+2) = **22** = `tl: 22` ✓ (وBBED: kref@+3، mref@+5، hrid@+7، nrid@+13، col@+19 ✓)
- deptx `RESEARCH`/`DALLAS`: 4 + 9 + 7 = **20** = `tl: 20` ✓
- empx SMITH (6 أعمدة): 4 + 4+6+6+4+8+3 = **35** = `tl: 35` ✓
- عضو محذوف: 4 = `tl: 4` ✓
- **ترتيب البايت** لـ curc/comc = ترتيب الملف (تعديل BBED لبايت `comc` المنخفض على ملف little-endian غيّر قيمته من 5 إلى 6).
- `cki` = فهرس صف المفتاح **داخل `kdbt[0]`** ("The value of cki is a pointer to the rownumber in tab[0]").
- ترتيب أعمدة المقطع: أعمدة المفتاح أولاً ثم أعمدة العضو؛ التحويل إلى `COL#` عبر `COL$.SEGCOL#` (L-6). **حالة دقيقة عالجناها:** عمود مفتاح أخير NULL لا يُخزَّن ← نحشو بـ NULL حتى `CLUCOLS` قبل إلحاق أعمدة العضو، وإلا تنزاح كل الأعمدة.
- علم `kdbh` `0x01` (`-------K`) **غير موثوق**: ظهر مرفوعاً في تفريغة ومنخفضاً في أخرى لكتل cluster ← لا نعتمد عليه.
- غير منشور بعد، وبالتالي `Unsupported`: قطع cluster المتسلسلة، cluster مع row dependencies، hash clusters.

---

## 4. ثغرات أخرى مسجّلة (ليست بعد في الكود)

| الثغرة | ملاحظة | المالك |
|---|---|---|
| `perm_9ir2` + `bindmp` (فكّ الضغط) | صف الرموز يبدأ بعدّاد استخدام u16 ثم cc؛ الصف المضغوط `2c 00 01 08 00` = flag lock cc(1 مرجع) ← رمز؛ `0xC8+len` بادئة حرفية | M5 |
| الأعمدة غير المستخدمة / المحذوفة | `COL$.PROPERTY 0x8000` = unused (البيانات باقية في الصف)، `0x4000` = dropped ← يجب احتسابها في `SEGCOL#` وإلا تنزاح الأعمدة | L-6 |
| `INTCOL#` مقابل `SEGCOL#` | الأعمدة الافتراضية (`0x10000`) ليس لها `SEGCOL#` (= 0) ← لا تُقرأ من الصف | L-6 |
| `CHARSETFORM` 2 = NCHAR | يستخدم الترميز الوطني (AL16UTF16) — جاهز في `Charsets{national}` | L-6 |
| علم `seg$.spare1` للضغط | `0x800` مضغوط، `0x1000000` OLTP، `0x2000000..0x8000000` HCC ← نعرف مسبقاً أي المقاطع نحتاج لها فكّ ضغط | L-6 |
| `tab$.property 0x400` | جدول cluster ← نقرأ كتل الـ cluster بـ `tab#` | L-6 |

---

## 5. ما لم أتحقق منه (شفافية)
- **موضع `nrid` بالنسبة لـ `dscn`** في القطع المتسلسلة مع ROWDEPENDENCIES: لا تفريغة منشورة ← `Unsupported` حتى يولّدها المختبر (حالة مطلوبة في L2).
- صيغة القطع المتسلسلة لصفوف cluster (صف أكبر من الكتلة) ← `Unsupported`.
- مواقع `kcvfh` بعد @140 لإصدارات 12c/19c/23ai.
- البحث بالروسية واليابانية والكورية لم يُضف معلومة ثنائية جديدة (نتائج مفاهيمية فقط عن row chaining/migration تطابق ما عندنا).

## 6. المصادر
- tomcoding, *Oracle DUL 程序设计（节选）*, 2017 — http://www.tomcoding.com/down/dul_design.pdf
- 惠星星, *14_深入解析Oracle table cluster结构*, itpub 2020 — http://m.blog.itpub.net/31442014/viewspace-2680174/
- *Oracle数据库入门之探究cluster table*, zhizuobiao 2018 — https://m.zhizuobiao.com/oracle/oracle-18122800379/
- *Clustering table 存储结构深入分析*, chinaunix — http://blog.chinaunix.net/uid-20708886-id-4025464.html
- *[20121021]探究表的rowdependencies属性*, aliyun — https://developer.aliyun.com/article/281639
- *bbed 找回被删除数据*, orasos — https://www.orasos.com/bbed-找回被删除数据.html
- *ORACLE的数据段压缩技术*, CSDN/itpub — https://blog.csdn.net/cnqc11810/article/details/100260183
- *Como funciona o Advanced Row Compression?*, commitandconquer 2025 — https://commitandconquer.com.br/2025/03/28/como-funciona-o-advanced-row-compression/
- *ORCLE 12C 增加列,无默认值*, orasos 2012 — https://www.orasos.com/orcle-12c-增加列无默认值.html
- Tencent Cloud Q&A 114456101 (SYS_NC + SYS_OP_VECBIT) — https://developer.cloud.tencent.com/ask/sof/114456101
- StackOverflow 32847399 (add-column optimisation, `PROPERTY & 1073741824`) — https://stackoverflow.com/questions/32847399
- aravinddarla 2019, *Drop Hidden Columns in Oracle table* — http://aravinddarla.blogspot.com/2019/07/drop-hidden-columns-in-oracle-table.html
- الجولة الأولى: orasos (DUL 12c CDB)، atoracle.cn (kcvfh BBED)، enmotech (tab$ في C_OBJ#)، itpub (أعلام الصفوف).
