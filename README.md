# Based on the apparently unmaintained [mime crate](https://github.com/hyperium/mime)

# mime

[![Build Status](https://travis-ci.org/morr0ne/neo-mime.svg?branch=main)](https://travis-ci.org/morr0ne/neo-mime)
[![crates.io](https://img.shields.io/crates/v/mime.svg)](https://crates.io/crates/mime)
[![docs.rs](https://docs.rs/neo-mime/badge.svg)](https://docs.rs/neo-mime)

Support MIME (HTTP Media Types) as strong types in Rust.

[Documentation](https://docs.rs/neo-mime)

## Usage

```rust
extern crate mime;

fn main() {
    // common types are constants
    let text = mime::TEXT_PLAIN;

    // deconstruct Mimes to match on them
    match (text.type_(), text.subtype()) {
        (mime::TEXT, mime::PLAIN) => {
            // plain text!
        },
        (mime::TEXT, _) => {
            // structured text!
        },
        _ => {
            // not text!
        }
    }
}
```
