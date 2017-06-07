//! # Mime
//!
//! Mime is now Media Type, technically, but `Mime` is more immediately
//! understandable, so the main type here is `Mime`.
//!
//! ## What is Mime?
//!
//! Example mime string: `text/plain`
//!
//! ```
//! let plain_text: mime::Mime = "text/plain".parse().unwrap();
//! assert_eq!(plain_text, mime::TEXT_PLAIN);
//! ```
//!
//! ## Inspecting Mimes
//!
//! ```
//! let mime = mime::TEXT_PLAIN;
//! match (mime.type_(), mime.subtype()) {
//!     (mime::TEXT, mime::PLAIN) => println!("plain text!"),
//!     (mime::TEXT, _) => println!("structured text"),
//!     _ => println!("not text"),
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/mime")]
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]


extern crate unicase;

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

mod parse;

/// A parsed mime or media type.
#[derive(Clone)]
pub struct Mime {
    source: Source,
    slash: usize,
    plus: Option<usize>,
    params: Params,
}

/// A section of a `Mime`.
///
/// For instance, for the Mime `image/svg+xml`, it contains 3 `Name`s,
/// `image`, `svg`, and `xml`.
///
/// In most cases, `Name`s are compared ignoring case.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name<'a> {
    // TODO: optimize with an Atom-like thing
    // There a `const` Names, and so it is possible for the statis strings
    // to havea different memory address. Additionally, when used in match
    // statements, the strings are compared with a memcmp, possibly even
    // if the address and length are the same.
    //
    // Being an enum with an Atom variant that is a usize (and without a
    // string pointer and boolean) would allow for faster comparisons.
    source: &'a str,
    insensitive: bool,
}

/// An error when parsing a `Mime` from a string.
#[derive(Debug)]
pub struct FromStrError {
    inner: parse::ParseError,
}

#[derive(Clone)]
enum Source {
    Atom(u8, &'static str),
    Dynamic(String),
}

impl Source {
    fn as_ref(&self) -> &str {
        match *self {
            Source::Atom(_, s) => s,
            Source::Dynamic(ref s) => s,
        }
    }
}

#[derive(Clone)]
enum Params {
    Utf8(usize),
    Custom(usize, Vec<(Str, Str)>),
    None,
}

#[derive(Clone, Copy)]
struct Str(usize, usize);

impl Mime {
    /// Get the top level media type for this `Mime`.
    ///
    /// # Example
    ///
    /// ```
    /// let mime = mime::TEXT_PLAIN;
    /// assert_eq!(mime.type_(), "text");
    /// assert_eq!(mime.type_(), mime::TEXT);
    /// ```
    #[inline]
    pub fn type_(&self) -> Name {
        Name {
            source: &self.source.as_ref()[..self.slash],
            insensitive: true,
        }
    }

    /// Get the subtype of this `Mime`.
    ///
    /// # Example
    ///
    /// ```
    /// let mime = mime::TEXT_PLAIN;
    /// assert_eq!(mime.subtype(), "plain");
    /// assert_eq!(mime.subtype(), mime::PLAIN);
    /// ```
    #[inline]
    pub fn subtype(&self) -> Name {
        let end = self.plus.unwrap_or_else(|| {
            return self.semicolon().unwrap_or(self.source.as_ref().len())
        });
        Name {
            source: &self.source.as_ref()[self.slash + 1..end],
            insensitive: true,
        }
    }

    /// Get an optional +suffix for this `Mime`.
    ///
    /// # Example
    ///
    /// ```
    /// let svg = "image/svg+xml".parse::<mime::Mime>().unwrap();
    /// assert_eq!(svg.suffix(), Some(mime::XML));
    /// assert_eq!(svg.suffix().unwrap(), "xml");
    ///
    ///
    /// assert!(mime::TEXT_PLAIN.suffix().is_none());
    /// ```
    #[inline]
    pub fn suffix(&self) -> Option<Name> {
        let end = self.semicolon().unwrap_or(self.source.as_ref().len());
        self.plus.map(|idx| Name {
            source: &self.source.as_ref()[idx + 1..end],
            insensitive: true,
        })
    }

    /// Look up a parameter by name.
    ///
    /// # Example
    ///
    /// ```
    /// let mime = mime::TEXT_PLAIN_UTF_8;
    /// assert_eq!(mime.get_param(mime::CHARSET), Some(mime::UTF_8));
    /// assert_eq!(mime.get_param("charset").unwrap(), "utf-8");
    /// assert!(mime.get_param("boundary").is_none());
    ///
    /// let mime = "multipart/form-data; boundary=ABCDEFG".parse::<mime::Mime>().unwrap();
    /// assert_eq!(mime.get_param(mime::BOUNDARY).unwrap(), "ABCDEFG");
    /// ```
    pub fn get_param<'a, N>(&'a self, attr: N) -> Option<Name<'a>>
    where N: PartialEq<Name<'a>> {
        match self.params {
            Params::Utf8(_) => {
                if attr == CHARSET {
                    Some(UTF_8)
                } else {
                    None
                }
            },
            Params::Custom(_, ref params) => {
                for &(ref name, ref value) in params {
                    let s = Name {
                        source: &self.source.as_ref()[name.0..name.1],
                        insensitive: true,
                    };
                    if attr == s {
                        return Some(Name {
                            source: &self.source.as_ref()[value.0..value.1],
                            insensitive: attr == CHARSET,
                        });
                    }
                }
                None
            },
            Params::None => None,
        }
    }

    #[inline]
    fn semicolon(&self) -> Option<usize> {
        match self.params {
            Params::Utf8(i) |
            Params::Custom(i, _) => Some(i),
            Params::None => None,
        }
    }

    fn atom(&self) -> u8 {
        match self.source {
            Source::Atom(a, _) => a,
            _ => 0,
        }
    }
}

// Mime ============

fn mime_eq_str(mime: &Mime, s: &str) -> bool {
    if let Params::Utf8(semicolon) = mime.params {
        if mime.source.as_ref().len() == s.len() {
            unicase::eq_ascii(mime.source.as_ref(), s)
        } else {
            params_eq(semicolon, mime.source.as_ref(), s)
        }
    } else if let Some(semicolon) = mime.semicolon() {
        params_eq(semicolon, mime.source.as_ref(), s)
    } else {
        unicase::eq_ascii(mime.source.as_ref(), s)
    }
}

fn params_eq(semicolon: usize, a: &str, b: &str) -> bool {
    if b.len() < semicolon + 1 {
        false
    } else if !unicase::eq_ascii(&a[..semicolon], &b[..semicolon]) {
        false
    } else {
        // gotta check for quotes, LWS, and for case senstive names
        let mut a = &a[semicolon + 1..];
        let mut b = &b[semicolon + 1..];
        let mut sensitive;

        loop {
            a = a.trim();
            b = b.trim();

            match (a.is_empty(), b.is_empty()) {
                (true, true) => return true,
                (true, false) |
                (false, true) => return false,
                (false, false) => (),
            }

            //name
            if let Some(a_idx) = a.find('=') {
                let a_name = a[..a_idx].trim_left();
                if let Some(b_idx) = b.find('=') {
                    let b_name = b[..b_idx].trim_left();
                    if !unicase::eq_ascii(a_name, b_name) {
                        return false;
                    }
                    sensitive = a_name != CHARSET;
                    a = &a[..a_idx];
                    b = &b[..b_idx];
                } else {
                    return false;
                }
            } else {
                return false;
            }
            //value
            let a_quoted = if a.as_bytes()[0] == b'"' {
                a = &a[1..];
                true
            } else {
                false
            };
            let b_quoted = if b.as_bytes()[0] == b'"' {
                b = &b[1..];
                true
            } else {
                false
            };

            let a_end = if a_quoted {
                if let Some(quote) = a.find('"') {
                    quote
                } else {
                    return false;
                }
            } else {
                a.find(';').unwrap_or(a.len())
            };

            let b_end = if b_quoted {
                if let Some(quote) = b.find('"') {
                    quote
                } else {
                    return false;
                }
            } else {
                b.find(';').unwrap_or(b.len())
            };

            if sensitive {
                if !unicase::eq_ascii(&a[..a_end], &b[..b_end]) {
                    return false;
                }
            } else {
                if &a[..a_end] != &b[..b_end] {
                    return false;
                }
            }
            a = &a[a_end..];
            b = &b[b_end..];
        }
    }
}

impl PartialEq for Mime {
    #[inline]
    fn eq(&self, other: &Mime) -> bool {
        match (self.atom(), other.atom()) {
            // TODO:
            // This could optimize for when there are no customs parameters.
            // Any parsed mime has already been lowercased, so if there aren't
            // any parameters that are case sensistive, this can skip the
            // unicase::eq_ascii, and just use a memcmp instead.
            (0, _) |
            (_, 0) => mime_eq_str(self, other.source.as_ref()),
            (a, b) => a == b,
        }
    }
}

impl Eq for Mime {}

impl PartialOrd for Mime {
    fn partial_cmp(&self, other: &Mime) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Mime {
    fn cmp(&self, other: &Mime) -> Ordering {
        self.source.as_ref().cmp(other.source.as_ref())
    }
}

impl Hash for Mime {
    fn hash<T: Hasher>(&self, hasher: &mut T) {
        hasher.write(self.source.as_ref().as_bytes());
    }
}

impl<'a> PartialEq<&'a str> for Mime {
    #[inline]
    fn eq(&self, s: & &'a str) -> bool {
        mime_eq_str(self, *s)
    }
}

impl<'a> PartialEq<Mime> for &'a str {
    #[inline]
    fn eq(&self, mime: &Mime) -> bool {
        mime_eq_str(mime, *self)
    }
}

impl FromStr for Mime {
    type Err = FromStrError;

    fn from_str(s: &str) -> Result<Mime, Self::Err> {
        parse::parse(s).map_err(|e| FromStrError { inner: e })
    }
}

impl AsRef<str> for Mime {
    #[inline]
    fn as_ref(&self) -> &str {
        self.source.as_ref()
    }
}

impl fmt::Debug for Mime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.source.as_ref(), f)
    }
}

impl fmt::Display for Mime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.source.as_ref(), f)
    }
}

// Name ============

fn name_eq_str(name: &Name, s: &str) -> bool {
    if name.insensitive {
        unicase::eq_ascii(name.source, s)
    } else {
        name.source == s
    }
}

impl<'a, 'b> PartialEq<&'b str> for Name<'a> {
    #[inline]
    fn eq(&self, other: & &'b str) -> bool {
        name_eq_str(self, *other)
    }
}

impl<'a, 'b> PartialEq<Name<'a>> for &'b str {
    #[inline]
    fn eq(&self, other: &Name<'a>) -> bool {
        name_eq_str(other, *self)
    }
}

impl<'a> AsRef<str> for Name<'a> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.source
    }
}

impl<'a> fmt::Debug for Name<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.source, f)
    }
}

impl<'a> fmt::Display for Name<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.source, f)
    }
}

macro_rules! names {
    ($($id:ident, $e:expr;)*) => (
        $(
        #[doc = $e]
        pub const $id: Name<'static> = Name {
            source: $e,
            insensitive: true,
        };
        )*

        #[test]
        fn test_names_macro_consts() {
            use std::ascii::AsciiExt;
            $(
            assert_eq!($id.source.to_ascii_lowercase(), $id.source);
            )*
        }
    )
}

names! {
    STAR, "*";

    TEXT, "text";
    IMAGE, "image";
    AUDIO, "audio";
    VIDEO, "video";
    APPLICATION, "application";
    MULTIPART, "multipart";
    MESSAGE, "message";
    MODEL, "model";

    // common text/ *
    PLAIN, "plain";
    HTML, "html";
    XML, "xml";
    JAVASCRIPT, "javascript";
    CSS, "css";
    EVENT_STREAM, "event-stream";

    // common application/*
    JSON, "json";
    WWW_FORM_URLENCODED, "x-www-form-urlencoded";
    MSGPACK, "msgpack";
    OCTET_STREAM, "octet-stream";

    // multipart/*
    FORM_DATA, "form-data";

    // common image/*
    PNG, "png";
    GIF, "gif";
    BMP, "bmp";
    JPEG, "jpeg";

    // audio/*
    MPEG, "mpeg";
    MP4, "mp4";
    OGG, "ogg";

    // parameters
    CHARSET, "charset";
    BOUNDARY, "boundary";
    UTF_8, "utf-8";
}

macro_rules! mimes {
    ($($id:ident, $($piece:tt),*;)*) => (
        #[allow(non_camel_case_types)]
        enum __Atoms {
            __Dynamic,
        $(
            $id,
        )*
        }

        $(
            mime_constant! {
                $id, $($piece),*
            }
        )*

        #[test]
        fn test_mimes_macro_consts() {
            [
            $(
            mime_constant_test! {
                $id, $($piece),*
            }
            ),*
            ].iter().enumerate().map(|(pos, &atom)| {
                assert_eq!(pos + 1, atom as usize, "atom {} in position {}", atom, pos + 1);
            }).collect::<Vec<()>>();
        }
    )
}

macro_rules! mime_constant {
    ($id:ident, $src:expr, $slash:expr) => (
        mime_constant!($id, $src, $slash, None);
    );
    ($id:ident, $src:expr, $slash:expr, $plus:expr) => (
        mime_constant!(FULL $id, $src, $slash, $plus, Params::None);
    );

    ($id:ident, $src:expr, $slash:expr, $plus:expr, $params:expr) => (
        mime_constant!(FULL $id, $src, $slash, $plus, Params::Utf8($params));
    );


    (FULL $id:ident, $src:expr, $slash:expr, $plus:expr, $params:expr) => (
        #[doc = $src]
        pub const $id: Mime = Mime {
            source: Source::Atom(__Atoms::$id as u8, $src),
            slash: $slash,
            plus: $plus,
            params: $params,
        };
    )
}


#[cfg(test)]
macro_rules! mime_constant_test {
    ($id:ident, $src:expr, $slash:expr) => (
        mime_constant_test!($id, $src, $slash, None);
    );
    ($id:ident, $src:expr, $slash:expr, $plus:expr) => (
        mime_constant_test!(FULL $id, $src, $slash, $plus, Params::None);
    );

    ($id:ident, $src:expr, $slash:expr, $plus:expr, $params:expr) => (
        mime_constant_test!(FULL $id, $src, $slash, $plus, Params::Utf8($params));
    );

    (FULL $id:ident, $src:expr, $slash:expr, $plus:expr, $params:expr) => ({
        let __mime = $id;
        let __slash = __mime.as_ref().as_bytes()[$slash];
        assert_eq!(__slash, b'/', "{:?} has {:?} at slash position {:?}", __mime, __slash as char, $slash);
        if let Some(plus) = __mime.plus {
            let __c = __mime.as_ref().as_bytes()[plus];
            assert_eq!(__c, b'+', "{:?} has {:?} at plus position {:?}", __mime, __c as char, plus);
        } else {
            assert!(!__mime.as_ref().as_bytes().contains(&b'+'), "{:?} forgot plus", __mime);
        }
        if let Params::Utf8(semicolon) = __mime.params {
            assert_eq!(__mime.as_ref().as_bytes()[semicolon], b';');
            assert_eq!(&__mime.as_ref()[semicolon..], "; charset=utf-8");
        } else if let Params::None = __mime.params {
            assert!(!__mime.as_ref().as_bytes().contains(&b';'));
        } else {
            unreachable!();
        }
        __mime.atom()
    })
}


mimes! {
    STAR_STAR, "*/*", 1;

    TEXT_PLAIN, "text/plain", 4;
    TEXT_PLAIN_UTF_8, "text/plain; charset=utf-8", 4, None, 10;
    TEXT_HTML, "text/html", 4;
    TEXT_CSS, "text/css", 4;
    TEXT_JAVSCRIPT, "text/javascript", 4;
    TEXT_XML, "text/xml", 4;
    TEXT_EVENT_STREAM, "text/event-stream", 4;

    IMAGE_JPEG, "image/jpeg", 5;
    IMAGE_GIF, "image/gif", 5;
    IMAGE_PNG, "image/png", 5;
    IMAGE_BMP, "image/bmp", 5;

    APPLICATION_JSON, "application/json", 11;
    APPLICATION_WWW_FORM_URLENCODED, "application/x-www-form-urlencoded", 11;
    APPLICATION_OCTET_STREAM, "application/octet-stream", 11;
    APPLICATION_MSGPACK, "application/msgpack", 11;

    MULTIPART_FORM_DATA, "multipart/form-data", 9;
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use super::*;

    #[test]
    fn test_type_() {
        assert_eq!(TEXT_PLAIN.type_(), TEXT);
    }


    #[test]
    fn test_subtype() {
        assert_eq!(TEXT_PLAIN.subtype(), PLAIN);
        assert_eq!(TEXT_PLAIN_UTF_8.subtype(), PLAIN);
        let mime = Mime::from_str("text/html+xml").unwrap();
        assert_eq!(mime.subtype(), HTML);
    }

    #[test]
    fn test_matching() {
        match (TEXT_PLAIN.type_(), TEXT_PLAIN.subtype()) {
            (TEXT, PLAIN) => (),
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_suffix() {
        assert_eq!(TEXT_PLAIN.suffix(), None);
        let mime = Mime::from_str("text/html+xml").unwrap();
        assert_eq!(mime.suffix(), Some(XML));
    }

    #[test]
    fn test_mime_fmt() {
        let mime = TEXT_PLAIN;
        assert_eq!(mime.to_string(), "text/plain");
        let mime = TEXT_PLAIN_UTF_8;
        assert_eq!(mime.to_string(), "text/plain; charset=utf-8");
    }

    #[test]
    fn test_mime_from_str() {
        assert_eq!(Mime::from_str("text/plain").unwrap(), TEXT_PLAIN);
        assert_eq!(Mime::from_str("TEXT/PLAIN").unwrap(), TEXT_PLAIN);
        assert_eq!(Mime::from_str("text/plain; charset=utf-8").unwrap(), TEXT_PLAIN_UTF_8);
        assert_eq!(Mime::from_str("text/plain;charset=\"utf-8\"").unwrap(), TEXT_PLAIN_UTF_8);

        let upper = Mime::from_str("TEXT/PLAIN").unwrap();
        assert_eq!(upper, TEXT_PLAIN);
        assert_eq!(upper.type_(), TEXT);
        assert_eq!(upper.subtype(), PLAIN);


        let extended = Mime::from_str("TEXT/PLAIN; CHARSET=UTF-8; FOO=BAR").unwrap();
        assert_eq!(extended, "text/plain; charset=utf-8; foo=BAR");
        assert_eq!(extended.get_param("charset").unwrap(), "utf-8");
        assert_eq!(extended.get_param("foo").unwrap(), "BAR");


        // stars
        assert_eq!("*/*".parse::<Mime>().unwrap(), STAR_STAR);
        assert_eq!("image/*".parse::<Mime>().unwrap(), "image/*");
        assert_eq!("text/*; charset=utf-8".parse::<Mime>().unwrap(), "text/*; charset=utf-8");

        // parse errors
        assert!("*/png".parse::<Mime>().is_err());
        assert!("*image/png".parse::<Mime>().is_err());
        assert!("text/*plain".parse::<Mime>().is_err());
    }

    #[test]
    fn test_case_sensitive_values() {
        let mime = Mime::from_str("multipart/form-data; charset=BASE64; boundary=ABCDEFG").unwrap();
        assert_eq!(mime.get_param(CHARSET).unwrap(), "bAsE64");
        assert_eq!(mime.get_param(BOUNDARY).unwrap(), "ABCDEFG");
        assert_ne!(mime.get_param(BOUNDARY).unwrap(), "abcdefg");
    }

    #[test]
    fn test_get_param() {
        assert_eq!(TEXT_PLAIN.get_param("charset"), None);
        assert_eq!(TEXT_PLAIN.get_param("baz"), None);

        assert_eq!(TEXT_PLAIN_UTF_8.get_param("charset"), Some(UTF_8));
        assert_eq!(TEXT_PLAIN_UTF_8.get_param("baz"), None);

        let mime = Mime::from_str("text/plain; charset=utf-8; foo=bar").unwrap();
        assert_eq!(mime.get_param(CHARSET).unwrap(), "utf-8");
        assert_eq!(mime.get_param("foo").unwrap(), "bar");
        assert_eq!(mime.get_param("baz"), None);


        let mime = Mime::from_str("text/plain;charset=\"utf-8\"").unwrap();
        assert_eq!(mime.get_param(CHARSET), Some(UTF_8));
    }

    #[test]
    fn test_name_eq() {
        assert_eq!(TEXT, TEXT);
        assert_eq!(TEXT, "text");
        assert_eq!("text", TEXT);
        assert_eq!(TEXT, "TEXT");

        let param = Name {
            source: "ABC",
            insensitive: false,
        };

        assert_eq!(param, param);
        assert_eq!(param, "ABC");
        assert_eq!("ABC", param);
        assert_ne!(param, "abc");
        assert_ne!("abc", param);
    }
}
