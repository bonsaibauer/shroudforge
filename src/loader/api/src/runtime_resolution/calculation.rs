//! Bounded, owned-data model of the attribute programs present in both builds.
//! This does not call arbitrary engine code or synthesize a native ABI.
//! Ordering, f32 rounding, wrapping integer arithmetic and ScaleToNewMax's
//! reference write are checked against the original x64 interpreters in Unicorn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scalar {
    Signed,
    Unsigned,
    Float,
}

impl Scalar {
    pub(crate) fn parse(name: &str) -> Result<Self, String> {
        match name.strip_prefix("keen::").unwrap_or(name) {
            "sint32" => Ok(Self::Signed),
            "uint32" => Ok(Self::Unsigned),
            "float32" => Ok(Self::Float),
            _ => Err("unsupported attribute calculation scalar".into()),
        }
    }
}

#[derive(Clone, Copy)]
enum Slot {
    Value(u32),
    Reference(usize),
}

fn pop(stack: &mut Vec<Slot>) -> Result<u32, String> {
    match stack.pop() {
        Some(Slot::Value(v)) => Ok(v),
        Some(Slot::Reference(_)) => Err("reference used as a scalar".into()),
        None => Err("attribute calculation stack underflow".into()),
    }
}

fn float(bits: u32) -> Result<f32, String> {
    let value = f32::from_bits(bits);
    if !value.is_finite() {
        return Err("non-finite attribute calculation value".into());
    }
    Ok(value)
}

fn cvttss2si(value: f32) -> i32 {
    if !(-2147483648.0..2147483648.0).contains(&value) {
        i32::MIN
    } else {
        value as i32
    }
}

/// A failure never modifies the supplied values. Empty programs retain storage.
pub(crate) fn evaluate(
    kind: Scalar,
    values: &mut [u32],
    words: &[u32],
) -> Result<Option<u32>, String> {
    if values.len() > 1024 || words.len() > 1024 {
        return Err("attribute calculation exceeds bounds".into());
    }
    if words.is_empty() {
        return Ok(None);
    }
    let mut working = values.to_vec();
    let mut stack = Vec::with_capacity(16);
    let mut pc = 0;
    while pc < words.len() {
        let word = words[pc];
        pc += 1;
        let opcode = word >> 16;
        let operand = (word & 0xffff) as usize;
        let value = match opcode {
            1 => 0,
            2 => *working
                .get(operand)
                .ok_or("attribute Load index outside root")?,
            3 if kind == Scalar::Signed => {
                if operand >= working.len() {
                    return Err("attribute LoadRef index outside root".into());
                }
                stack.push(Slot::Reference(operand));
                if stack.len() > 16 {
                    return Err("attribute stack exceeds proven local capacity".into());
                }
                continue;
            }
            5 => {
                let literal = *words.get(pc).ok_or("truncated attribute Push literal")?;
                pc += 1;
                // Float VM uses MOV eax; CVTSI2SS xmm6,rax. The immediate is
                // an unsigned integer converted to float, NOT IEEE float bits.
                if kind == Scalar::Float {
                    (literal as f32).to_bits()
                } else {
                    literal
                }
            }
            6 | 7 | 8 => {
                let right = pop(&mut stack)?;
                let left = pop(&mut stack)?;
                if kind == Scalar::Float {
                    let left = float(left)?;
                    let right = float(right)?;
                    let result = match opcode {
                        6 => left + right,
                        7 => left - right,
                        _ => left * right,
                    };
                    if !result.is_finite() {
                        return Err("non-finite attribute calculation result".into());
                    }
                    result.to_bits()
                } else {
                    match opcode {
                        6 => left.wrapping_add(right),
                        7 => left.wrapping_sub(right),
                        _ => left.wrapping_mul(right),
                    }
                }
            }
            12 => {
                let bound2 = pop(&mut stack)?;
                let bound1 = pop(&mut stack)?;
                let value = pop(&mut stack)?;
                // The engine sorts the two bounds; reversed limits are legal.
                match kind {
                    Scalar::Signed => (value as i32).clamp(
                        (bound1 as i32).min(bound2 as i32),
                        (bound1 as i32).max(bound2 as i32),
                    ) as u32,
                    Scalar::Unsigned => value.clamp(bound1.min(bound2), bound1.max(bound2)),
                    Scalar::Float => {
                        let a = float(bound1)?;
                        let b = float(bound2)?;
                        let v = float(value)?;
                        // Preserve v's sign bit when equal (including -0).
                        if v < a.min(b) {
                            a.min(b).to_bits()
                        } else if v > a.max(b) {
                            a.max(b).to_bits()
                        } else {
                            value
                        }
                    }
                }
            }
            13 if kind == Scalar::Signed => {
                let new_max = pop(&mut stack)? as i32;
                let old_max = pop(&mut stack)? as i32;
                let minimum = pop(&mut stack)? as i32;
                let Some(Slot::Reference(index)) = stack.pop() else {
                    return Err("ScaleToNewMax requires LoadRef".into());
                };
                let old_range = old_max.wrapping_sub(minimum);
                if new_max != old_max && old_range != 0 {
                    let old_value = (working[index] as i32).wrapping_sub(minimum);
                    let ratio = old_value as f32 / old_range as f32;
                    let scaled = ratio * new_max.wrapping_sub(minimum) as f32;
                    let mut updated = minimum.wrapping_add(cvttss2si(scaled));
                    if updated == 0 && old_value != 0 {
                        updated = 1;
                    }
                    working[index] = updated as u32;
                }
                new_max as u32
            }
            15 => {
                let right = pop(&mut stack)?;
                let left = pop(&mut stack)?;
                match kind {
                    Scalar::Signed => (left as i32).max(right as i32) as u32,
                    Scalar::Unsigned => left.max(right),
                    Scalar::Float => {
                        if float(left)? > float(right)? {
                            left
                        } else {
                            right
                        }
                    }
                }
            }
            _ => return Err(format!("unverified attribute opcode {opcode} for {kind:?}")),
        };
        if kind == Scalar::Float {
            float(value)?;
        }
        stack.push(Slot::Value(value));
        if stack.len() > 16 {
            return Err("attribute stack exceeds proven local capacity".into());
        }
    }
    let result = pop(&mut stack)?;
    if !stack.is_empty() {
        return Err("attribute calculation leaves unused stack values".into());
    }
    values.copy_from_slice(&working);
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_and_failure_are_atomic() {
        let mut values = [12, 30];
        assert!(evaluate(Scalar::Signed, &mut values, &[0x20002]).is_err());
        assert!(evaluate(Scalar::Signed, &mut values, &[0x5ffff]).is_err());
        assert!(evaluate(Scalar::Signed, &mut values, &[0x6ffff]).is_err());
        assert!(evaluate(Scalar::Float, &mut [f32::INFINITY.to_bits()], &[0x20000]).is_err());
        // A real reference write followed by an unknown instruction rolls back.
        assert!(
            evaluate(
                Scalar::Signed,
                &mut values,
                &[
                    0x30000, 0x5ffff, 0, 0x5ffff, 30, 0x5ffff, 60, 0xdffff, 0x16ffff
                ]
            )
            .is_err()
        );
        assert_eq!(values, [12, 30]);
    }
    #[test]
    fn float_push_is_a_numeric_integer_conversion() {
        // Native Revive_Delay vector: 2*2 clamped to [2,10] is 4. Treating
        // literal 10 as float bits would incorrectly clamp the result to 2.
        let mut values = [2f32.to_bits(); 6];
        assert_eq!(
            evaluate(
                Scalar::Float,
                &mut values,
                &[0x20003, 0x20004, 0x8ffff, 0x20005, 0x5ffff, 10, 0xcffff]
            )
            .unwrap(),
            Some(4f32.to_bits())
        );
    }
    #[test]
    #[ignore = "set SHROUDFORGE_TEST_VM to independently emulated original-engine vectors"]
    fn original_engine_differential() {
        let path = std::env::var("SHROUDFORGE_TEST_VM").unwrap();
        let data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let vectors = data["vectors"].as_array().unwrap();
        for v in vectors {
            let mut values: Vec<u32> = serde_json::from_value(v["values"].clone()).unwrap();
            let words: Vec<u32> = serde_json::from_value(v["words"].clone()).unwrap();
            let result = evaluate(
                Scalar::parse(v["kind"].as_str().unwrap()).unwrap(),
                &mut values,
                &words,
            );
            assert_eq!(v["status"], 0, "native failure: {v}");
            assert_eq!(
                result.unwrap(),
                Some(v["result"].as_u64().unwrap() as u32),
                "{v}"
            );
            assert_eq!(serde_json::json!(values), v["after"], "{v}");
        }
        println!(
            "{} independent original-engine differential vectors matched",
            vectors.len()
        );
    }
}
