//! RFC 6381 `codecs` parameter strings, as used in HLS and DASH manifests and in
//! `MediaSource.isTypeSupported`.

use filmcraft_isobmff as iso;

/// The RFC 6381 string for a sample entry, e.g. `avc1.64001F`, `hvc1.1.6.L93.B0`, `av01.0.04M.08`,
/// `mp4a.40.2`, `Opus`. `None` for codecs without a registered string.
pub(crate) fn codec_string(e: &iso::SampleEntry) -> Option<String> {
    use iso::CodecConfig as C;
    Some(match &e.codec {
        C::Avc(c) => format!("avc1.{:02X}{:02X}{:02X}", c.profile, c.compatibility, c.level),
        C::Hevc(c) => hevc(c),
        C::Av1(c) => {
            let depth = if c.twelve_bit {
                12
            } else if c.high_bitdepth {
                10
            } else {
                8
            };
            format!("av01.{}.{:02}{}.{:02}", c.seq_profile, c.seq_level_idx_0, if c.seq_tier_0 { 'H' } else { 'M' }, depth)
        }
        C::Vp9(c) => format!("vp09.{:02}.{:02}.{:02}", c.profile, c.level, c.bit_depth),
        C::Aac(c) => format!("mp4a.40.{}", if c.object_type == 0 { 2 } else { c.object_type }),
        C::Mp3 => "mp4a.6B".to_string(),
        C::Opus(_) => "Opus".to_string(),
        C::Flac(_) => "fLaC".to_string(),
        C::Ac3 { .. } => "ac-3".to_string(),
        C::Eac3 { .. } => "ec-3".to_string(),
        C::ProRes { fourcc } | C::Jpeg { fourcc } | C::Dnx { fourcc } => String::from_utf8_lossy(&fourcc.0).to_string(),
        C::Apv(_) => "apv1".to_string(),
        _ => return None,
    })
}

/// ISO/IEC 14496-15 Annex E.3: `hvc1.<space><idc>.<compat reversed hex>.<tier><level>.<constraints>`.
fn hevc(c: &iso::HevcConfig) -> String {
    let space = match c.general_profile_space {
        1 => "A",
        2 => "B",
        3 => "C",
        _ => "",
    };
    let compat = c.general_profile_compatibility_flags.reverse_bits();
    let tier = if c.general_tier_flag { 'H' } else { 'L' };
    let bytes = c.general_constraint_indicator_flags.to_be_bytes();
    // the 48-bit constraint flags are the low six bytes; trailing zero bytes are dropped
    let mut tail: Vec<u8> = bytes.get(2..).unwrap_or(&[]).to_vec();
    while tail.last() == Some(&0) {
        tail.pop();
    }
    let mut s = format!("hvc1.{space}{}.{:X}.{tier}{}", c.general_profile_idc, compat, c.general_level_idc);
    for b in tail {
        s.push_str(&format!(".{b:02X}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avc_string_is_profile_compat_level() {
        let cfg = iso::AvcConfig { profile: 0x64, compatibility: 0, level: 0x1F, ..iso::AvcConfig::new(vec![vec![0x67]], vec![vec![0x68]], 4) };
        let e = iso::SampleEntry::avc(cfg, 64, 48);
        assert_eq!(codec_string(&e).as_deref(), Some("avc1.64001F"));
    }

    #[test]
    fn aac_lc_is_mp4a_40_2() {
        let e = iso::SampleEntry::aac(vec![0x12, 0x10], 2, 44_100);
        assert_eq!(codec_string(&e).as_deref(), Some("mp4a.40.2"));
    }

    #[test]
    fn hevc_main_matches_the_annex_example() {
        // Main profile, level 3.1 (93), compat flag 1 and 2 set: hvc1.1.6.L93.B0
        let cfg = iso::HevcConfig {
            general_profile_idc: 1,
            general_profile_compatibility_flags: 0x6000_0000,
            general_level_idc: 93,
            general_constraint_indicator_flags: 0xB000_0000_0000,
            ..Default::default()
        };
        assert_eq!(hevc(&cfg), "hvc1.1.6.L93.B0");
    }
}
