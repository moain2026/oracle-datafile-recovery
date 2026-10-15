//! Write a synthetic damaged image: `cargo run -p odr-testkit --example make_image -- out.img 1024`.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use odr_core::ByteOrder;
use odr_testkit::{Corruption, corrupt, synth_image};

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("path");
    let n: usize = a.next().map_or(1024, |s| s.parse().unwrap());
    let mut img = synth_image(n, 8192, ByteOrder::Little, 7);
    corrupt(&mut img, 8192, 5, Corruption::BitRot, 1);
    corrupt(&mut img, 8192, 9, Corruption::Encrypt, 2);
    std::fs::write(path, img).unwrap();
}
