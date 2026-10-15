# المساهمة
- الفروع: `feat/…` `fix/…` `docs/…` · Commits: [Conventional Commits](https://www.conventionalcommits.org/) `feat: …` `fix: …`
- كل PR: يربط متطلباً (REQ-…) أو مشكلة، يمر بالاختبارات، يحدّث CHANGELOG وSTATUS.
- كل محلّل بنية جديدة (kcbh، kdbr، NUMBER…) يأتي مع: اختبار وحدة على بايتات من المجموعة الذهبية + هدف fuzz.
- كل تغيير يمسّ الأداء يرفق قياس `bench/` قبل/بعد.
- تعريف "مكتمل": كود + اختبار + fuzz + توثيق + STATUS محدّث.
