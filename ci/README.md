# تفعيل CI (خطوة واحدة على المالك)
الوكيل لا يملك صلاحية `workflows` على GitHub، لذا ملف CI محفوظ هنا. لتفعيله انقله مرة واحدة:
```
git mv ci/github-ci.yml .github/workflows/ci.yml && git commit -m "ci: enable" && git push
```
أو من واجهة GitHub: Add file ← `.github/workflows/ci.yml` ← الصق محتوى `ci/github-ci.yml`.
يشغّل: fmt · clippy -D warnings · test · doc · build release على **Windows وLinux**، cargo-deny، fuzz دقيقة لكل هدف، ويرفع `odr.exe` كـ artifact.
