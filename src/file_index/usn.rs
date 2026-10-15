use anyhow::{Context, Result, ensure};
#[derive(Clone, Debug)]
pub(super) struct Record {
    pub(super) id: u64,
    pub(super) parent: u64,
    pub(super) usn: i64,
    pub(super) reason: u32,
    pub(super) name: String,
}

fn u16le(b: &[u8], n: usize) -> u16 {
    u16::from_le_bytes(b[n..n + 2].try_into().unwrap())
}
fn u32le(b: &[u8], n: usize) -> u32 {
    u32::from_le_bytes(b[n..n + 4].try_into().unwrap())
}
fn u64le(b: &[u8], n: usize) -> u64 {
    u64::from_le_bytes(b[n..n + 8].try_into().unwrap())
}

pub(super) fn parse(bytes: &[u8]) -> Result<(u64, Vec<Record>)> {
    ensure!(bytes.len() >= 8, "truncated response header");
    let next = u64le(bytes, 0);
    let mut pos = 8;
    let mut records = Vec::new();
    while pos < bytes.len() {
        ensure!(bytes.len() - pos >= 8, "truncated USN header");
        let size = u32le(bytes, pos) as usize;
        ensure!(
            size >= 60 && size <= bytes.len() - pos && size.is_multiple_of(8),
            "invalid USN record length"
        );
        let b = &bytes[pos..pos + size];
        ensure!(
            u16le(b, 4) == 2 && u16le(b, 6) == 0,
            "unsupported USN record version {}.{}; rebuild with compatible reader required",
            u16le(b, 4),
            u16le(b, 6)
        );
        let len = u16le(b, 56) as usize;
        let offset = u16le(b, 58) as usize;
        ensure!(
            offset >= 60
                && offset.is_multiple_of(2)
                && len.is_multiple_of(2)
                && offset + len <= size,
            "invalid USN filename bounds"
        );
        let utf16: Vec<u16> = b[offset..offset + len]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        let name = String::from_utf16(&utf16)
            .context("unsupported non-Unicode filename; refusing silent loss")?;
        ensure!(
            !name.is_empty() && !name.contains(['\\', '/', '\0']),
            "invalid filename component"
        );
        records.push(Record {
            id: u64le(b, 8),
            parent: u64le(b, 16),
            usn: u64le(b, 24) as i64,
            reason: u32le(b, 40),
            name,
        });
        pos += size;
    }
    Ok((next, records))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_and_unknown_records_are_rejected() {
        for b in [vec![], vec![0; 7], vec![0; 16]] {
            assert!(parse(&b).is_err());
        }
        let mut b = vec![0u8; 72];
        b[8..12].copy_from_slice(&64u32.to_le_bytes());
        b[12..14].copy_from_slice(&3u16.to_le_bytes());
        assert!(parse(&b).unwrap_err().to_string().contains("unsupported"));
        b[12..14].copy_from_slice(&2u16.to_le_bytes());
        b[64..66].copy_from_slice(&2u16.to_le_bytes());
        b[66..68].copy_from_slice(&60u16.to_le_bytes());
        b[68..70].copy_from_slice(&(b'a' as u16).to_le_bytes());
        assert_eq!(parse(&b).unwrap().1[0].name, "a");
        b[64..66].copy_from_slice(&100u16.to_le_bytes());
        assert!(parse(&b).is_err());
    }
}
