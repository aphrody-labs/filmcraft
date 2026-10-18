//! Codec configuration translation between MP4 sample entries and Matroska tracks.
//!
//! The ISO codecs share their configuration records between the two containers (`avcC`, `hvcC`,
//! `av1C`, the AAC `AudioSpecificConfig`), so those are copied byte for byte. Opus and FLAC differ
//! in byte order and framing and are rebuilt; AC-3 and VP9 records that Matroska does not store
//! are derived from the first packet.

use std::sync::Arc;

use filmcraft_isobmff as iso;
use filmcraft_matroska as mkv;
use filmcraft_time::FrameRate;

use crate::container::{ColorSignal, FrameIndex, HdrStatic, TrackInfo, TrackKind};
use crate::error::{Error, Result};
use crate::source::{Adapter, ByteSource};

fn dim(v: u32, what: &str) -> Result<u16> {
    u16::try_from(v).map_err(|_| Error::unsupported(format!("{what} {v} does not fit an MP4 sample entry")))
}

/// Build the MP4 sample entry for a Matroska track.
pub(crate) fn entry_from_matroska(
    t: &mkv::Track,
    info: &TrackInfo,
    file: &mkv::MkvFile,
    src: &Arc<dyn ByteSource>,
    index: &FrameIndex,
) -> Result<iso::SampleEntry> {
    use mkv::Codec;
    let first_packet = || -> Result<Vec<u8>> {
        if index.is_empty() {
            return Err(Error::unsupported("empty track"));
        }
        Ok(file.read_sample(&Adapter(src.clone()), info.index, 0)?)
    };
    let video = |e: iso::SampleEntry| -> iso::SampleEntry { with_video_extras(e, info, t) };
    let (w, h) = (info.width, info.height);
    match &t.codec {
        Codec::Avc { avcc } => Ok(video(iso::SampleEntry::avc(iso::AvcConfig::parse(avcc)?, dim(w, "width")?, dim(h, "height")?))),
        Codec::Hevc { hvcc } => Ok(video(iso::SampleEntry::hevc(iso::HevcConfig::parse(hvcc)?, dim(w, "width")?, dim(h, "height")?))),
        Codec::Av1 { av1c } => Ok(video(iso::SampleEntry::av1(iso::Av1Config::parse(av1c)?, dim(w, "width")?, dim(h, "height")?))),
        Codec::Vp9 { private } => {
            let cfg = vp9_config(private, t, &first_packet()?, info)?;
            Ok(video(iso::SampleEntry::video(iso::FourCc(*b"vp09"), iso::CodecConfig::Vp9(cfg), dim(w, "width")?, dim(h, "height")?)))
        }
        Codec::ProRes { fourcc } => Ok(video(iso::SampleEntry::prores(iso::FourCc(fourcc.unwrap_or(*b"apch")), dim(w, "width")?, dim(h, "height")?))),
        Codec::Mjpeg => Ok(video(iso::SampleEntry::jpeg(dim(w, "width")?, dim(h, "height")?))),
        Codec::Apv { apvc } => Ok(video(iso::SampleEntry::apv(iso::ApvConfig::parse(apvc)?, dim(w, "width")?, dim(h, "height")?))),
        Codec::Aac { asc } => Ok(iso::SampleEntry::aac(asc.clone(), info.channels, info.sample_rate)),
        Codec::Opus { head } => {
            let cfg = opus_config(head)?;
            let ch = u32::from(cfg.output_channels);
            Ok(iso::SampleEntry::audio(iso::FourCc(*b"Opus"), iso::CodecConfig::Opus(cfg), ch, 48_000.0, 16))
        }
        Codec::Flac { private } => {
            let blocks = private.strip_prefix(b"fLaC").unwrap_or(private);
            let mut full = vec![0u8; 4];
            full.extend_from_slice(blocks);
            let cfg = iso::FlacConfig::parse(&full)?;
            let (ch, rate, bits) = (u32::from(cfg.channels), f64::from(cfg.sample_rate), u16::from(cfg.bits_per_sample));
            Ok(iso::SampleEntry::audio(iso::FourCc(*b"fLaC"), iso::CodecConfig::Flac(cfg), ch, rate, bits))
        }
        Codec::Mp3 | Codec::Mp2 => Ok(iso::SampleEntry::audio(iso::FourCc(*b"mp4a"), iso::CodecConfig::Mp3, info.channels, f64::from(info.sample_rate), 16)),
        Codec::Ac3 => {
            let dac3 = dac3_from_frame(&first_packet()?)?;
            Ok(iso::SampleEntry::audio(iso::FourCc(*b"ac-3"), iso::CodecConfig::Ac3 { dac3 }, info.channels, f64::from(info.sample_rate), 16))
        }
        Codec::Pcm { float, big_endian, bits } => {
            let cfg = iso::PcmConfig {
                bits: *bits,
                float: *float,
                big_endian: *big_endian,
                signed: *bits != 8,
                channels: info.channels,
                sample_rate: f64::from(info.sample_rate),
            };
            Ok(iso::SampleEntry::pcm(cfg))
        }
        other => Err(Error::unsupported(format!("Matroska codec {} has no MP4 mapping", other.name()))),
    }
}

/// Colour, pixel aspect and HDR metadata of a Matroska video track carried onto the MP4 entry.
fn with_video_extras(mut e: iso::SampleEntry, info: &TrackInfo, t: &mkv::Track) -> iso::SampleEntry {
    if let Some(v) = e.video.as_mut() {
        if let Some(c) = info.color
            && (c.primaries, c.transfer, c.matrix) != (2, 2, 2)
        {
            v.color = Some(iso::ColorInfo::Nclx { primaries: c.primaries, transfer: c.transfer, matrix: c.matrix, full_range: c.full_range });
        }
        if info.pixel_aspect != (1, 1) {
            v.pixel_aspect = Some(info.pixel_aspect);
        }
        if let Some(h) = &info.hdr {
            let chroma = |x: f32| (x * 50_000.0).round().clamp(0.0, 65_535.0) as u16;
            if let (Some(p), Some(w), Some(max), Some(min)) = (h.primaries, h.white_point, h.max_luminance, h.min_luminance) {
                v.mastering_display = Some(iso::MasteringDisplay {
                    primaries: p.map(|(x, y)| (chroma(x), chroma(y))),
                    white_point: (chroma(w.0), chroma(w.1)),
                    max_luminance: (max * 10_000.0).round().clamp(0.0, u32::MAX as f32) as u32,
                    min_luminance: (min * 10_000.0).round().clamp(0.0, u32::MAX as f32) as u32,
                });
            }
            if let (Some(a), Some(b)) = (h.max_cll, h.max_fall) {
                v.content_light = Some((a.min(65_535) as u16, b.min(65_535) as u16));
            }
        }
    }
    let _ = t;
    e
}

fn opus_config(head: &[u8]) -> Result<iso::OpusConfig> {
    let bad = || Error::invalid("OpusHead is too short");
    if head.get(..8) != Some(b"OpusHead") {
        return Err(Error::invalid("CodecPrivate is not an OpusHead"));
    }
    let le16 = |o: usize| head.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(bad);
    let channels = *head.get(9).ok_or_else(bad)?;
    let rate = head.get(12..16).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(bad)?;
    Ok(iso::OpusConfig {
        output_channels: channels,
        pre_skip: le16(10)?,
        input_sample_rate: rate,
        output_gain: le16(16)? as i16,
        channel_mapping_family: *head.get(18).ok_or_else(bad)?,
        channel_mapping: head.get(19..).unwrap_or(&[]).to_vec(),
    })
}

fn opus_head(c: &iso::OpusConfig) -> Vec<u8> {
    let mut v = b"OpusHead".to_vec();
    v.push(1);
    v.push(c.output_channels);
    v.extend_from_slice(&c.pre_skip.to_le_bytes());
    v.extend_from_slice(&c.input_sample_rate.to_le_bytes());
    v.extend_from_slice(&c.output_gain.to_le_bytes());
    v.push(c.channel_mapping_family);
    v.extend_from_slice(&c.channel_mapping);
    v
}

/// `dac3` from the first AC-3 syncframe (ETSI TS 102 366 §4.3 and Annex F.4).
fn dac3_from_frame(frame: &[u8]) -> Result<Vec<u8>> {
    let bad = || Error::invalid("AC-3 syncframe header too short");
    if frame.get(..2) != Some(&[0x0B, 0x77]) {
        return Err(Error::invalid("first AC-3 packet does not start with a syncword"));
    }
    let b = frame.get(4..8).ok_or_else(bad)?;
    let fscod = b[0] >> 6;
    let frmsizecod = b[0] & 0x3F;
    let bsid = b[1] >> 3;
    let bsmod = b[1] & 7;
    let acmod = b[2] >> 5;
    // optional mix levels sit between acmod and lfeon
    let mut bit = 3;
    if acmod & 1 != 0 && acmod != 1 {
        bit += 2;
    }
    if acmod & 4 != 0 {
        bit += 2;
    }
    if acmod == 2 {
        bit += 2;
    }
    let all = u32::from_be_bytes([b[2], b[3], 0, 0]) >> 16;
    let lfeon = ((all >> (16 - bit - 1)) & 1) as u8;
    let bit_rate_code = frmsizecod >> 1;
    let v: u32 = (u32::from(fscod) << 22)
        | (u32::from(bsid) << 17)
        | (u32::from(bsmod) << 14)
        | (u32::from(acmod) << 11)
        | (u32::from(lfeon) << 10)
        | (u32::from(bit_rate_code) << 5);
    Ok(vec![(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// A `vpcC` for a VP9 track: from the Matroska `CodecPrivate` feature list when present, else from
/// the uncompressed header of the first keyframe (VP9 bitstream spec §6.2).
fn vp9_config(private: &[u8], t: &mkv::Track, first: &[u8], info: &TrackInfo) -> Result<iso::VpcConfig> {
    let mut cfg = iso::VpcConfig {
        profile: 0,
        level: 0,
        bit_depth: 8,
        chroma_subsampling: 1,
        full_range: false,
        colour_primaries: 2,
        transfer_characteristics: 2,
        matrix_coefficients: 2,
        codec_init: Vec::new(),
    };
    let mut have = false;
    // CodecPrivate: repeated (id u8, length u8, value): 1 profile, 2 level, 3 bit depth, 4 chroma subsampling
    let mut p = private;
    while let [id, len, rest @ ..] = p {
        let len = usize::from(*len);
        let Some(v) = rest.get(..len) else { break };
        let val = v.first().copied().unwrap_or(0);
        match id {
            1 => {
                cfg.profile = val;
                have = true;
            }
            2 => cfg.level = val,
            3 => cfg.bit_depth = val,
            4 => cfg.chroma_subsampling = val,
            _ => {}
        }
        p = rest.get(len..).unwrap_or(&[]);
    }
    if !have {
        let h = vp9_keyframe_header(first)?;
        cfg.profile = h.0;
        cfg.bit_depth = h.1;
        cfg.chroma_subsampling = h.2;
    }
    let _ = t;
    if let Some(c) = info.color {
        cfg.colour_primaries = u8::try_from(c.primaries).unwrap_or(2);
        cfg.transfer_characteristics = u8::try_from(c.transfer).unwrap_or(2);
        cfg.matrix_coefficients = u8::try_from(c.matrix).unwrap_or(2);
        cfg.full_range = c.full_range;
    }
    Ok(cfg)
}

/// (profile, bit depth, chroma subsampling code) from a VP9 keyframe's uncompressed header.
fn vp9_keyframe_header(d: &[u8]) -> Result<(u8, u8, u8)> {
    struct Bits<'a>(&'a [u8], usize);
    impl Bits<'_> {
        fn get(&mut self, n: usize) -> Option<u32> {
            let mut v = 0u32;
            for _ in 0..n {
                let byte = *self.0.get(self.1 / 8)?;
                v = (v << 1) | u32::from((byte >> (7 - (self.1 % 8))) & 1);
                self.1 += 1;
            }
            Some(v)
        }
    }
    let err = || Error::invalid("cannot read the VP9 keyframe header");
    let mut b = Bits(d, 0);
    if b.get(2).ok_or_else(err)? != 2 {
        return Err(err());
    }
    let lo = b.get(1).ok_or_else(err)?;
    let hi = b.get(1).ok_or_else(err)?;
    let profile = (hi << 1 | lo) as u8;
    if profile == 3 {
        b.get(1).ok_or_else(err)?;
    }
    if b.get(1).ok_or_else(err)? == 1 {
        return Err(err()); // show_existing_frame
    }
    let frame_type = b.get(1).ok_or_else(err)?;
    b.get(2).ok_or_else(err)?; // show_frame, error_resilient_mode
    if frame_type != 0 {
        return Err(Error::invalid("the first VP9 packet is not a keyframe"));
    }
    if b.get(24).ok_or_else(err)? != 0x49_83_42 {
        return Err(err());
    }
    let bit_depth = if profile >= 2 { if b.get(1).ok_or_else(err)? == 1 { 12 } else { 10 } } else { 8 };
    let color_space = b.get(3).ok_or_else(err)?;
    let sub = if color_space != 7 {
        b.get(1).ok_or_else(err)?; // color_range
        if profile == 1 || profile == 3 {
            let sx = b.get(1).ok_or_else(err)?;
            let sy = b.get(1).ok_or_else(err)?;
            match (sx, sy) {
                (1, 0) => 2,
                (0, 0) => 3,
                _ => 1,
            }
        } else {
            1
        }
    } else {
        3
    };
    Ok((profile, bit_depth, sub))
}

/// Everything a Matroska writer needs for one track, from the MP4 sample entry.
pub(crate) fn spec_from_entry(entry: &iso::SampleEntry, info: &TrackInfo, webm: bool) -> Result<mkv::TrackSpec> {
    use iso::CodecConfig as C;
    let video = |id: &str, private: Vec<u8>| -> Result<mkv::TrackSpec> {
        let mut s = mkv::TrackSpec::new(mkv::TrackKind::Video, id);
        s.codec_private = private;
        let (w, h) = (info.width, info.height);
        s.video_size = Some((w, h));
        if info.pixel_aspect.0 > 0 && info.pixel_aspect.1 > 0 && info.pixel_aspect != (1, 1) {
            // display size w*n x h*d reduced: its ratio, with the coded size, gives the pixel aspect back exactly
            let (dw, dh) = (u64::from(w) * u64::from(info.pixel_aspect.0), u64::from(h) * u64::from(info.pixel_aspect.1));
            let g = gcd(dw, dh).max(1);
            if let (Ok(dw), Ok(dh)) = (u32::try_from(dw / g), u32::try_from(dh / g)) {
                s.display_size = Some((dw, dh));
            }
        }
        s.colour = colour_from(info.color, info.hdr.as_ref());
        if let Some(FrameRate { num, den }) = info.frame_rate.filter(|_| info.constant_frame_rate)
            && num > 0
            && den > 0
        {
            s.default_duration_ns = u64::try_from(i128::from(den) * 1_000_000_000 / i128::from(num)).ok();
        }
        Ok(s)
    };
    let audio = |id: &str, private: Vec<u8>, bits: Option<u64>| -> mkv::TrackSpec {
        let mut s = mkv::TrackSpec::new(mkv::TrackKind::Audio, id);
        s.codec_private = private;
        s.audio = Some((f64::from(info.sample_rate), u64::from(info.channels), bits));
        s
    };
    let spec = match &entry.codec {
        C::Avc(c) => video("V_MPEG4/ISO/AVC", c.to_bytes())?,
        C::Hevc(c) => video("V_MPEGH/ISO/HEVC", c.to_bytes())?,
        C::Av1(c) => video("V_AV1", c.to_bytes())?,
        C::Vp9(_) => video("V_VP9", Vec::new())?,
        C::ProRes { fourcc } => video("V_PRORES", fourcc.0.to_vec())?,
        C::Jpeg { .. } => video("V_MJPEG", Vec::new())?,
        C::Apv(c) => video("V_APV", c.to_bytes())?,
        C::Aac(c) => audio("A_AAC", c.asc.clone(), None),
        C::Opus(o) => {
            let mut s = audio("A_OPUS", opus_head(o), None);
            s.audio = Some((48_000.0, u64::from(o.output_channels), None));
            s.codec_delay_ns = u64::from(o.pre_skip) * 1_000_000_000 / 48_000;
            s.seek_pre_roll_ns = 80_000_000;
            s
        }
        C::Flac(f) => {
            let mut private = b"fLaC".to_vec();
            private.extend_from_slice(&f.metadata_blocks);
            audio("A_FLAC", private, Some(u64::from(f.bits_per_sample)))
        }
        C::Mp3 => audio("A_MPEG/L3", Vec::new(), None),
        C::Ac3 { .. } => audio("A_AC3", Vec::new(), None),
        C::Eac3 { .. } => audio("A_EAC3", Vec::new(), None),
        C::Alac { cookie } => audio("A_ALAC", cookie.clone(), None),
        C::Pcm(p) => {
            let id = if p.float {
                "A_PCM/FLOAT/IEEE"
            } else if p.big_endian {
                "A_PCM/INT/BIG"
            } else {
                "A_PCM/INT/LIT"
            };
            audio(id, Vec::new(), Some(u64::from(p.bits)))
        }
        other => return Err(Error::unsupported(format!("{} has no Matroska mapping", other.name()))),
    };
    if webm {
        let ok = matches!(entry.codec, C::Vp9(_) | C::Av1(_) | C::Opus(_));
        if !ok {
            return Err(Error::unsupported(format!("WebM carries VP8, VP9, AV1, Opus and Vorbis; {} cannot go into it (use Matroska)", entry.codec.name())));
        }
    }
    Ok(spec)
}

fn colour_from(c: Option<ColorSignal>, hdr: Option<&HdrStatic>) -> Option<mkv::Colour> {
    if c.is_none() && hdr.is_none() {
        return None;
    }
    let mut o = mkv::Colour::default();
    if let Some(c) = c {
        o.primaries = Some(u64::from(c.primaries));
        o.transfer_characteristics = Some(u64::from(c.transfer));
        o.matrix_coefficients = Some(u64::from(c.matrix));
        o.range = Some(if c.full_range { 2 } else { 1 });
    }
    if let Some(h) = hdr {
        o.max_cll = h.max_cll.map(u64::from);
        o.max_fall = h.max_fall.map(u64::from);
        if let (Some(p), Some(w), Some(max), Some(min)) = (h.primaries, h.white_point, h.max_luminance, h.min_luminance) {
            o.mastering = Some(mkv::MasteringMetadata {
                primaries: p.map(|(x, y)| (f64::from(x), f64::from(y))),
                white_point: (f64::from(w.0), f64::from(w.1)),
                luminance_max: f64::from(max),
                luminance_min: f64::from(min),
            });
        }
    }
    Some(o)
}

/// Whether a track of `kind` with codec `name` can be copied into WebM.
pub(crate) fn webm_allows(name: &str, kind: TrackKind) -> bool {
    match kind {
        TrackKind::Video => matches!(name, "vp8" | "vp9" | "av1"),
        TrackKind::Audio => matches!(name, "opus" | "vorbis"),
        _ => false,
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}
