//! Oracle internal datatype decoders.
//!
//! Contract for every decoder in this module:
//! * Input is the raw column bytes (length byte already stripped).
//! * Output is either an exact value or a [`DecodeError`](crate::DecodeError) — never an
//!   approximation. Floats are never used for NUMBER.
//! * Decoders are pure, allocation-light and never panic (fuzzed).
//!
//! Internal type codes (OCI, \[O2\]): 1 `VARCHAR2`/`NVARCHAR2`, 2 `NUMBER`, 12 `DATE`,
//! 23 `RAW`, 69 `ROWID`, 96 `CHAR`/`NCHAR`, 100 `BINARY_FLOAT`, 101 `BINARY_DOUBLE`,
//! 112 CLOB/NCLOB, 113 BLOB, 180 TIMESTAMP, 181 TIMESTAMP WITH TIME ZONE,
//! 182 INTERVAL YEAR TO MONTH, 183 INTERVAL DAY TO SECOND,
//! 231 TIMESTAMP WITH LOCAL TIME ZONE.

pub mod date;
pub mod number;

pub use date::OraDate;
pub use number::OraNumber;
