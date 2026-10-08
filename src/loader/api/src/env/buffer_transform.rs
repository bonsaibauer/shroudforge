//! A deliberately small x64 proof checker, not a native-call dispatcher.
//! Accepted programs only load through two argument pointer slots, copy source
//! bytes to destination bytes, optionally set EAX, and return. Every instruction
//! through RET must be understood; calls, branches, globals and stack use fail.
use serde::Serialize;

#[derive(Clone, Copy, Debug)]
enum Value {
    Unknown,
    Slot(bool), // true: source, false: destination
    Buffer(bool),
    Bytes(usize, usize),
    Constant(u32),
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Copy {
    source_offset: usize,
    destination_offset: usize,
    size: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Plan {
    pub source_minimum: usize,
    pub destination_minimum: usize,
    pub code_length: usize,
    pub native_constant_result: Option<u32>,
    pub copies: Vec<Copy>,
}

impl Plan {
    pub fn apply(&self, source: &[u8], destination: &[u8]) -> Result<Vec<u8>, String> {
        if source.len() < self.source_minimum || destination.len() < self.destination_minimum {
            return Err(format!(
                "buffer too short: source needs {}, destination needs {} bytes",
                self.source_minimum, self.destination_minimum
            ));
        }
        if source.len() > 16 * 1024 * 1024 || destination.len() > 16 * 1024 * 1024 {
            return Err("owned buffer exceeds 16 MiB".into());
        }
        let mut result = destination.to_vec();
        for copy in &self.copies {
            result[copy.destination_offset..copy.destination_offset + copy.size]
                .copy_from_slice(&source[copy.source_offset..copy.source_offset + copy.size]);
        }
        Ok(result)
    }
}

pub(crate) fn prove(code: &[u8]) -> Option<Plan> {
    let code = &code[..code.len().min(512)];
    let mut registers = [Value::Unknown; 16];
    registers[1] = Value::Slot(false); // Windows x64 RCX -> destination pointer
    registers[2] = Value::Slot(true); // RDX -> source pointer
    let mut pos = 0;
    let mut plan = Plan {
        source_minimum: 0,
        destination_minimum: 0,
        code_length: 0,
        native_constant_result: None,
        copies: Vec::new(),
    };
    let byte = |pos: &mut usize| -> Option<u8> {
        let v = *code.get(*pos)?;
        *pos += 1;
        Some(v)
    };
    let volatile = |reg| matches!(reg, 0 | 1 | 2 | 8 | 9 | 10 | 11);
    loop {
        let mut op = byte(&mut pos)?;
        if op == 0xc3 {
            if plan.copies.is_empty() {
                return None;
            }
            plan.code_length = pos;
            plan.native_constant_result = match registers[0] {
                Value::Constant(v) => Some(v),
                _ => None,
            };
            return Some(plan);
        }
        if op == 0xb8 {
            // mov eax, imm32 (no REX, no alternate register)
            let bytes: [u8; 4] = code.get(pos..pos + 4)?.try_into().ok()?;
            pos += 4;
            registers[0] = Value::Constant(u32::from_le_bytes(bytes));
            continue;
        }
        let rex = if (0x40..=0x4f).contains(&op) {
            let rex = op;
            op = byte(&mut pos)?;
            rex
        } else {
            0
        };
        if rex & 2 != 0 {
            return None;
        } // no extended index/SIB addressing
        let zero_extend = op == 0x0f;
        if zero_extend {
            op = byte(&mut pos)?;
            if !matches!(op, 0xb6 | 0xb7) {
                return None;
            }
        }
        let (load, width) = if zero_extend {
            (true, if op == 0xb6 { 1 } else { 2 })
        } else {
            match op {
                0x8b => (true, if rex & 8 != 0 { 8 } else { 4 }),
                0x89 => (false, if rex & 8 != 0 { 8 } else { 4 }),
                0x8a => (true, 1),
                0x88 => (false, 1),
                _ => return None,
            }
        };
        let modrm = byte(&mut pos)?;
        let mode = modrm >> 6;
        let reg = ((modrm >> 3 & 7) | ((rex & 4) << 1)) as usize;
        let base = ((modrm & 7) | ((rex & 1) << 3)) as usize;
        // Reject AH/CH/DH/BH, register operands, SIB, RIP-relative and absolute addresses.
        if !volatile(reg)
            || mode == 3
            || modrm & 7 == 4
            || (mode == 0 && modrm & 7 == 5)
            || (width == 1 && !zero_extend && rex == 0 && reg >= 4)
        {
            return None;
        }
        let offset = match mode {
            0 => 0,
            1 => i32::from(byte(&mut pos)? as i8),
            2 => {
                let v = i32::from_le_bytes(code.get(pos..pos + 4)?.try_into().ok()?);
                pos += 4;
                v
            }
            _ => return None,
        };
        if !(0..=65536).contains(&offset) {
            return None;
        }
        let offset = offset as usize;
        if load {
            registers[reg] = match registers[base] {
                Value::Slot(source) if offset == 0 && width == 8 && !zero_extend => {
                    Value::Buffer(source)
                }
                Value::Buffer(true) => {
                    plan.source_minimum = plan.source_minimum.max(offset + width);
                    Value::Bytes(offset, width)
                }
                _ => return None,
            };
            // An 8-bit register write preserves the high bits; don't model them.
            // Subsequent wider stores are rejected by the recorded byte width.
        } else {
            if !matches!(registers[base], Value::Buffer(false)) {
                return None;
            }
            let Value::Bytes(source_offset, available) = registers[reg] else {
                return None;
            };
            if width > available {
                return None;
            }
            plan.copies.push(Copy {
                source_offset,
                destination_offset: offset,
                size: width,
            });
            plan.destination_minimum = plan.destination_minimum.max(offset + width);
        }
    }
}

pub(crate) fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 || text.len() > 1024 {
        return None;
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let hi = (pair[0] as char).to_digit(16)?;
            let lo = (pair[1] as char).to_digit(16)?;
            Some((hi * 16 + lo) as u8)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    // Exact independently disassembled client/server Extinguish callback bodies.
    const PACK: &str = "4c8b02488b11498b4008488902418b4010894208410fb6401488420cb80d000000c3";
    const UNPACK: &str = "4c8b01488b0a488b01498940088b4108418940100fb6410c41884014c3";

    #[test]
    fn owned_copy_roundtrip_preserves_unwritten_bytes_and_rejects_short_buffers() {
        let pack = prove(&unhex(PACK).unwrap()).unwrap();
        let unpack = prove(&unhex(UNPACK).unwrap()).unwrap();
        let source: Vec<u8> = (0..32).collect();
        let packed = pack.apply(&source, &[0xa5; 13]).unwrap();
        assert_eq!(packed, source[8..21]);
        assert_eq!(pack.native_constant_result, Some(13));
        let unpacked = unpack.apply(&packed, &[0xa5; 32]).unwrap();
        assert_eq!(&unpacked[8..21], &source[8..21]);
        assert!(
            unpacked[..8]
                .iter()
                .chain(unpacked[21..].iter())
                .all(|b| *b == 0xa5)
        );
        assert!(pack.apply(&source[..20], &[0; 13]).is_err());
        assert!(pack.apply(&source, &[0; 12]).is_err());
    }

    #[test]
    fn rejects_unproved_control_flow_aliases_globals_widths_and_truncation() {
        let code = unhex(PACK).unwrap();
        for size in 0..code.len() {
            assert!(prove(&code[..size]).is_none());
        }
        for bad in [
            "e800000000c3",
            "eb00c3",
            "488b0500000000c3",
            "4c8b02488b11498910c3",
            "4c8b02488b11410fb600488902c3",
            "4c8b02488b11418a20408822c3",
        ] {
            assert!(prove(&unhex(bad).unwrap()).is_none(), "{bad}");
        }
    }
}
