#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VeinNativeNotices {
    pub bytes: u32,
    pub version: u32,
    pub flags: u32,
    pub toggle: u32,
    pub radius: u32,
    pub reserved: u32,
}
impl Default for VeinNativeNotices {
    fn default() -> Self {
        Self {
            bytes: 24,
            version: 1,
            flags: 0,
            toggle: 0,
            radius: 0,
            reserved: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VeinNoticeBatch {
    pub toggle: Option<bool>,
    pub radius: Option<u32>,
}
impl VeinNativeNotices {
    pub fn decode(self) -> Result<VeinNoticeBatch, i32> {
        if self.bytes != 24
            || self.version != 1
            || self.flags & !3 != 0
            || self.reserved != 0
            || self.toggle > 1
            || (self.flags & 1 == 0 && self.toggle != 0)
            || (self.flags & 2 != 0 && !(1..=3).contains(&self.radius))
            || (self.flags & 2 == 0 && self.radius != 0)
        {
            return Err(2);
        }
        Ok(VeinNoticeBatch {
            toggle: (self.flags & 1 != 0).then_some(self.toggle != 0),
            radius: (self.flags & 2 != 0).then_some(self.radius),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vein_native_notices_are_exact_and_optional_false_survives() {
        assert_eq!(std::mem::size_of::<VeinNativeNotices>(), 24);
        let mut n = VeinNativeNotices::default();
        assert_eq!(n.decode(), Ok(VeinNoticeBatch::default()));
        n.flags = 3;
        n.radius = 2;
        assert_eq!(
            n.decode(),
            Ok(VeinNoticeBatch {
                toggle: Some(false),
                radius: Some(2)
            })
        );
        n.toggle = 1;
        assert_eq!(n.decode().unwrap().toggle, Some(true));
    }
    #[test]
    fn vein_native_notices_reject_bad_schema_before_delivery() {
        for bad in [
            VeinNativeNotices {
                bytes: 23,
                ..Default::default()
            },
            VeinNativeNotices {
                version: 2,
                ..Default::default()
            },
            VeinNativeNotices {
                flags: 4,
                ..Default::default()
            },
            VeinNativeNotices {
                reserved: 1,
                ..Default::default()
            },
            VeinNativeNotices {
                toggle: 1,
                ..Default::default()
            },
            VeinNativeNotices {
                flags: 2,
                radius: 4,
                ..Default::default()
            },
        ] {
            assert_eq!(bad.decode(), Err(2));
        }
    }
}
