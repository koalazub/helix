//! Kitty graphics protocol implementation of [`GraphicsProtocol`].
//!
//! Reference: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>
//!
//! This editor primarily uses Kitty's Unicode placeholder protocol
//! (`U=1`): the image is transmitted once under a stable id, Kitty
//! caches it, and placeholder cells written into the normal text grid
//! reference it by id wherever they are drawn.

use std::io;

use super::GraphicsProtocol;

/// Zero-sized handle to the Kitty graphics protocol. Construct with
/// `KittyProtocol` and call trait methods directly — there is no state.
#[derive(Debug, Default, Clone, Copy)]
pub struct KittyProtocol;

impl GraphicsProtocol for KittyProtocol {
    fn write_delete(&self, id: u64, w: &mut dyn io::Write) -> io::Result<()> {
        // a=d (delete), d=I (delete by image id), i=<id>, q=2 (quiet).
        write!(w, "\x1b_Ga=d,d=I,i={},q=2\x1b\\", id)
    }

    fn extract_image_id(&self, payload: &str) -> Option<u64> {
        // The escape we care about looks like:
        //
        //     \x1b_Ga=T,f=100,t=d,q=2,U=1,i=1001,m=0;<base64>\x1b\\
        //
        // Only inspect the first APC escape — continuation chunks
        // (`m=1`) reuse the same id. The header is a comma-separated
        // list of `key=value` pairs terminated by `;`. We accept either
        // `i=` immediately after `\x1b_G` or anywhere in the parameter
        // list (the protocol does not specify an order).
        let apc_start = payload.find("\x1b_G")?;
        let rest = &payload[apc_start + 3..];
        let header_end = rest.find(';')?;
        let header = &rest[..header_end];

        for field in header.split(',') {
            if let Some(value) = field.strip_prefix("i=") {
                return value.parse::<u64>().ok();
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_id_from_virtual_placement() {
        let p = KittyProtocol;
        let payload = "\x1b_Ga=T,f=100,t=d,q=2,U=1,i=1001,m=0;abcd\x1b\\";
        assert_eq!(p.extract_image_id(payload), Some(1001));
    }

    #[test]
    fn extracts_id_from_direct_placement() {
        // Capital-I (`I=`) is the *client reference id*, not the image
        // id. extract_image_id must NOT match it.
        let p = KittyProtocol;
        let payload = "\x1b_Ga=T,f=100,t=d,q=2,I=42,r=12,m=0;abcd\x1b\\";
        assert_eq!(p.extract_image_id(payload), None);
    }

    #[test]
    fn returns_none_on_non_apc_payload() {
        let p = KittyProtocol;
        assert_eq!(p.extract_image_id("hello world"), None);
        assert_eq!(p.extract_image_id(""), None);
    }

    #[test]
    fn returns_none_on_unterminated_header() {
        let p = KittyProtocol;
        assert_eq!(p.extract_image_id("\x1b_Ga=T,i=5"), None);
    }

    #[test]
    fn ignores_bad_integer() {
        let p = KittyProtocol;
        let payload = "\x1b_Ga=T,i=notanumber;abcd\x1b\\";
        assert_eq!(p.extract_image_id(payload), None);
    }

    #[test]
    fn handles_id_at_start_of_parameter_list() {
        let p = KittyProtocol;
        let payload = "\x1b_Gi=7,a=T,U=1;abcd\x1b\\";
        assert_eq!(p.extract_image_id(payload), Some(7));
    }

    #[test]
    fn write_delete_emits_expected_escape() {
        let p = KittyProtocol;
        let mut buf = Vec::new();
        p.write_delete(42, &mut buf).unwrap();
        assert_eq!(buf, b"\x1b_Ga=d,d=I,i=42,q=2\x1b\\");
    }
}
