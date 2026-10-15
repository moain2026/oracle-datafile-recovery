# Oracle Datafile Recovery Platform

منصة مؤسسية لقراءة ملفات بيانات أوراكل (.dbf) التالفة أو غير القابلة للفتح واستخراج البيانات منها دون الحاجة إلى instance أوراكل — بوضع القاموس وبدونه، مع دعم ASM وLOB والضغط والصفوف المتسلسلة، وتقرير شرعي قياسي.

Enterprise-grade, DUL-class tool for extracting data from corrupted / unopenable Oracle datafiles without a running Oracle instance.

## Repository layout

| Path | Content |
|---|---|
| `STATUS.md` | الحالة وخارطة الطريق M0–M7 بمعايير إنجاز قابلة للقياس — **ابدأ هنا** |
| `AGENTS.md` / `CLAUDE.md` | قواعد العمل للمطورين والوكلاء |
| `docs/design/architecture.md` | المعمارية (C4): خط Scan → Extract، المكوّنات، المخاطر |
| `docs/adr/` | القرارات: لغة النواة، الدليل للقراءة فقط، clean-room، المجموعة الذهبية |
| `docs/requirements/` | المتطلبات بصيغة EARS (REQ-001 المسح، REQ-002 القاموس، REQ-003 الأنواع) |
| `docs/research/01_foundational_research_ar.md` | البحث التأسيسي: الأدوات القائمة، الأبحاث، بنية الكتل، الفجوات، خارطة الطريق (كل مرجع تم التحقق منه) |
| `docs/research/02_expert_interview_questions_ar.md` | أسئلة مقابلة خبير الإنتاج |
| `crates/` | النواة Rust: `odr-core` · `odr-cli` (`odr.exe`) · `odr-testkit` |
| `docs/agents/` | تقسيم العمل على وكيلين (Lead + Astra) وملف مهمة لكل منهما |
| `docs/runbooks/` `docs/postmortems/` | قوالب التشغيل وتحليل الحوادث |
| `LESSONS.md` `CHANGELOG.md` `CONTRIBUTING.md` `SECURITY.md` | الدروس، السجل، المساهمة، الأمان وبيانات العملاء |
| `src/` | Core engine (to be added) |
| `tests/fixtures/` | Reference datafiles with controlled corruption (to be added) |

## Status

v0.1 skeleton: core contracts, exact NUMBER/DATE decoding, reference block scanner, CI on Windows + Linux. Next: M0 lab + M1 scanner (Astra) in parallel with the data-block layer (Lead) — see `STATUS.md`, `docs/agents/`.

```
cargo build --release -p odr-cli
odr scan path\to\USERS01.DBF            # read-only; SHA-256 verified before/after
odr scan USERS01.DBF --json --block-size 8192 --endian little
```
