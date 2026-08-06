//! Test-for-test adaptation of upstream `test/uuid.test.ts`.

#[cfg(test)]
mod tests {
    fn parse_timestamp(uuid: &str) -> u64 {
        u64::from_str_radix(&uuid.replace('-', "")[..12], 16).unwrap()
    }

    #[test]
    fn uuidv7_uses_rfc_9562_layout_and_preserves_monotonic_order() {
        let first = crate::utils::uuidv7();
        let second = crate::utils::uuidv7();
        let third = crate::utils::uuidv7();
        for uuid in [&first, &second, &third] {
            assert_eq!(uuid.len(), 36);
            assert_eq!(&uuid[8..9], "-");
            assert_eq!(&uuid[13..14], "-");
            assert_eq!(&uuid[18..19], "-");
            assert_eq!(&uuid[23..24], "-");
            assert_eq!(&uuid[14..15], "7", "version nibble: {uuid}");
            assert!(
                matches!(&uuid[19..20], "8" | "9" | "a" | "b"),
                "variant nibble: {uuid}"
            );
            assert!(
                uuid.chars()
                    .all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
        }
        assert!(first < second, "{first} !< {second}");
        assert!(second < third, "{second} !< {third}");
        assert!(parse_timestamp(&third) >= parse_timestamp(&first));
    }
}
