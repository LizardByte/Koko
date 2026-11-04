//! Preserve sufficient thread stack space in packaged musl media tools.

use std::io::{
    self,
    Read,
    Seek,
    SeekFrom,
    Write,
};

const MINIMUM_STACK: u64 = 1024 * 1024;
const GNU_STACK: u32 = 0x6474_e551;

fn read_bytes<const N: usize>(input: &mut impl Read) -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    input.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn invalid_table() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid ELF64 media tool program header table",
    )
}

/// Raise musl's default thread stack without changing executable-stack permissions.
///
/// Older build-deps tools omit the minimum requested by FFmpeg's musl configuration.
/// Only the owned, extracted package is changed; user-supplied tools are left alone.
pub(crate) fn ensure_minimum_stack(input: &mut (impl Read + Seek + Write)) -> io::Result<()> {
    input.seek(SeekFrom::Start(0))?;
    if read_bytes::<6>(input)? != *b"\x7fELF\x02\x01" {
        return Err(invalid_table());
    }
    input.seek(SeekFrom::Start(32))?;
    let table = u64::from_le_bytes(read_bytes(input)?);
    input.seek(SeekFrom::Start(54))?;
    let entry_size = u64::from(u16::from_le_bytes(read_bytes(input)?));
    let entries = u64::from(u16::from_le_bytes(read_bytes(input)?));
    let length = input.seek(SeekFrom::End(0))?;
    let end = table
        .checked_add(entry_size * entries)
        .ok_or_else(invalid_table)?;
    if table < 64 || entry_size < 56 || end > length {
        return Err(invalid_table());
    }
    for index in 0..entries {
        let entry = table + index * entry_size;
        input.seek(SeekFrom::Start(entry))?;
        if u32::from_le_bytes(read_bytes(input)?) != GNU_STACK {
            continue;
        }
        input.seek(SeekFrom::Start(entry + 40))?;
        let stack = u64::from_le_bytes(read_bytes(input)?);
        if stack < MINIMUM_STACK {
            input.seek(SeekFrom::Start(entry + 40))?;
            input.write_all(&MINIMUM_STACK.to_le_bytes())?;
            input.flush()?;
        }
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Media tool has no GNU_STACK header",
    ))
}

#[cfg(test)]
mod tests {
    use super::ensure_minimum_stack;
    use std::io::Cursor;

    fn executable(
        machine: u16,
        stack: u64,
    ) -> Vec<u8> {
        let mut bytes = vec![0; 208];
        bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");
        bytes[18..20].copy_from_slice(&machine.to_le_bytes());
        bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
        bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
        bytes[56..58].copy_from_slice(&2_u16.to_le_bytes());
        bytes[64..68].copy_from_slice(&1_u32.to_le_bytes());
        bytes[120..124].copy_from_slice(&0x6474_e551_u32.to_le_bytes());
        bytes[124..128].copy_from_slice(&6_u32.to_le_bytes());
        bytes[160..168].copy_from_slice(&stack.to_le_bytes());
        bytes[176..].fill(0xa5);
        bytes
    }

    #[test]
    fn raises_both_musl_architectures_and_preserves_permissions_and_payload() {
        for machine in [62, 183] {
            let original = executable(machine, 0);
            let mut expected = original.clone();
            expected[160..168].copy_from_slice(&1_048_576_u64.to_le_bytes());
            let mut file = Cursor::new(original);
            ensure_minimum_stack(&mut file).unwrap();
            assert_eq!(file.get_ref(), &expected);
            ensure_minimum_stack(&mut file).unwrap();
            assert_eq!(file.into_inner(), expected);
        }
    }

    #[test]
    fn preserves_an_existing_sufficient_stack() {
        for stack in [1_048_576_u64, 8_388_608] {
            let original = executable(62, stack);
            let mut file = Cursor::new(original.clone());
            ensure_minimum_stack(&mut file).unwrap();
            assert_eq!(file.into_inner(), original);
        }
    }

    #[test]
    fn rejects_malformed_or_missing_headers_without_writing() {
        let original = executable(62, 0);
        let mut invalid = Vec::new();
        for offset in [0, 4, 5] {
            let mut bytes = original.clone();
            bytes[offset] = 0;
            invalid.push(bytes);
        }
        for table in [0_u64, u64::MAX] {
            let mut bytes = original.clone();
            bytes[32..40].copy_from_slice(&table.to_le_bytes());
            invalid.push(bytes);
        }
        let mut small_entry = original.clone();
        small_entry[54..56].copy_from_slice(&8_u16.to_le_bytes());
        invalid.push(small_entry);
        let mut missing_stack = original.clone();
        missing_stack[120..124].copy_from_slice(&2_u32.to_le_bytes());
        invalid.push(missing_stack);
        invalid.push(original[..140].to_vec());
        invalid.push(original[..6].to_vec());
        for bytes in invalid {
            let mut file = Cursor::new(bytes.clone());
            assert!(ensure_minimum_stack(&mut file).is_err());
            assert_eq!(file.into_inner(), bytes);
        }
    }
}
