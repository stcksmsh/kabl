//! Just enough of the wasm binary format to list, drop and add custom sections. Hand-rolled so
//! the CLI needs no wasm-tools dependency and the output stays byte-deterministic.

pub const META: &str = "kabl.meta";
pub const NOTICES: &str = "kabl.notices";

/// The licence notices for the toolchain code every module links (core, compiler_builtins, libm,
/// Unicode tables). `cargo kabl build` writes this into the `kabl.notices` custom section.
pub const NOTICE_TEXT: &str = include_str!("../../../sdk/kabl-module-sdk/NOTICE.txt");

const HEADER: &[u8] = b"\0asm\x01\0\0\0";

pub struct Section<'a> {
    pub id: u8,
    /// The whole section as it appears in the file: id, size and payload.
    pub raw: &'a [u8],
    pub payload: &'a [u8],
}

impl<'a> Section<'a> {
    /// Name and data, when this is a custom section.
    pub fn custom(&self) -> Option<(&'a str, &'a [u8])> {
        if self.id != 0 {
            return None;
        }
        let (len, n) = leb(self.payload)?;
        let rest = self.payload.get(n..)?;
        let name = std::str::from_utf8(rest.get(..len as usize)?).ok()?;
        Some((name, &rest[len as usize..]))
    }
}

fn leb(b: &[u8]) -> Option<(u32, usize)> {
    let mut v = 0u64;
    for (i, &x) in b.iter().take(5).enumerate() {
        v |= u64::from(x & 0x7f) << (7 * i);
        if x & 0x80 == 0 {
            return u32::try_from(v).ok().map(|v| (v, i + 1));
        }
    }
    None
}

fn put_leb(out: &mut Vec<u8>, mut v: u32) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

pub fn sections(wasm: &[u8]) -> Result<Vec<Section<'_>>, String> {
    if !wasm.starts_with(HEADER) {
        return Err("not a wasm module (bad header)".into());
    }
    let mut at = HEADER.len();
    let mut out = vec![];
    while at < wasm.len() {
        let id = wasm[at];
        let (size, n) = leb(&wasm[at + 1..]).ok_or("truncated section header")?;
        let start = at + 1 + n;
        let end = start
            .checked_add(size as usize)
            .filter(|&e| e <= wasm.len())
            .ok_or("section runs past the end of the file")?;
        out.push(Section {
            id,
            raw: &wasm[at..end],
            payload: &wasm[start..end],
        });
        at = end;
    }
    Ok(out)
}

/// The data of the custom section `name` (the first one), if present.
pub fn custom<'a>(wasm: &'a [u8], name: &str) -> Result<Option<&'a [u8]>, String> {
    Ok(sections(wasm)?
        .iter()
        .find_map(|s| s.custom().filter(|c| c.0 == name).map(|c| c.1)))
}

/// Keeps every real section and only the `kabl.meta` custom section (so `name`, `producers` and
/// `target_features` go), then appends `kabl.notices` carrying `notices`. The same input always
/// gives the same bytes.
pub fn finish(wasm: &[u8], notices: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = HEADER.to_vec();
    for s in sections(wasm)? {
        match s.custom() {
            None if s.id == 0 => return Err("malformed custom section".into()),
            Some((name, _)) if name != META => {}
            _ => out.extend_from_slice(s.raw),
        }
    }
    let mut payload = vec![];
    put_leb(&mut payload, NOTICES.len() as u32);
    payload.extend_from_slice(NOTICES.as_bytes());
    payload.extend_from_slice(notices);
    out.push(0);
    put_leb(&mut out, payload.len() as u32);
    out.extend_from_slice(&payload);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(customs: &[(&str, &[u8])]) -> Vec<u8> {
        let mut m = HEADER.to_vec();
        for (n, d) in customs {
            let mut p = vec![];
            put_leb(&mut p, n.len() as u32);
            p.extend_from_slice(n.as_bytes());
            p.extend_from_slice(d);
            m.push(0);
            put_leb(&mut m, p.len() as u32);
            m.extend_from_slice(&p);
        }
        m
    }

    #[test]
    fn finish_keeps_meta_drops_the_rest_and_adds_notices() {
        let m = module(&[("name", b"xx"), (META, b"kind a.b"), ("producers", b"yy")]);
        let f = finish(&m, b"NOTICE").unwrap();
        assert_eq!(custom(&f, META).unwrap(), Some(&b"kind a.b"[..]));
        assert_eq!(custom(&f, NOTICES).unwrap(), Some(&b"NOTICE"[..]));
        assert_eq!(custom(&f, "name").unwrap(), None);
        assert_eq!(finish(&f, b"NOTICE").unwrap(), f, "idempotent");
    }

    #[test]
    fn long_sections_use_multibyte_sizes() {
        let big = vec![b'x'; 5000];
        let f = finish(&module(&[(META, &big)]), &big).unwrap();
        assert_eq!(custom(&f, NOTICES).unwrap().unwrap().len(), 5000);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(sections(b"nope").is_err());
        let mut m = module(&[(META, b"x")]);
        m.truncate(m.len() - 1);
        assert!(sections(&m).is_err());
    }
}
