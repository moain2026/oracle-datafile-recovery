//! Data-dictionary mode (L-6, REQ-002). **Owner: Lead.**
//!
//! The chain is: `kcvfhrdb` (file 1 block 1) → `bootstrap$` segment →
//! `bootstrap$` rows (DDL text with hard-coded `OBJNO`, `TABNO`, `CLUSTER`
//! and `EXTENTS (FILE f BLOCK b)`) → segments of `OBJ$`, `TAB$`, `COL$`,
//! `USER$`, `TS$`… → user tables. This module starts at `bootstrap$`:
//! [`bootstrap`] turns its rows into a [`bootstrap::Catalog`] of core
//! objects and their storage. Every binary fact used here is in
//! `docs/design/lab-verify.md` (LV-025..027, LV-032..034).

pub mod bootstrap;
