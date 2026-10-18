# FilmCraft as a library

`filmcraft-lib` (`crates/lib`) is the supported way to use FilmCraft's media stack from another
program. It re-exports nothing from the editor: the dependency tree has no `engine`, `ui-egui`,
`gpu`, `speech` or `tts` crates (checked in the table below), no FFmpeg and no `unsafe`.

## What is reachable through the facade

| Capability | Formats | Entry point |
|---|---|---|
| Probe: tracks, codec, size, rate, colour, HDR, exact frame count | MP4/MOV, Matroska/WebM (packet table); every decodable format (decoder facts) | `probe_path`, `probe_bytes`, `Demuxer` |
| Exact packet / timestamp index, keyframes, time to frame | MP4/MOV, Matroska/WebM | `Demuxer::frame_index` |
| Seek + decode video | H.264, HEVC, VP8, VP9, AV1, APV, ProRes, DNxHD/HR, Motion-JPEG, MPEG-1/2 in MP4/MOV, Matroska/WebM, AVI, MXF, MPEG-TS/PS, Y4M, APV, animated GIF/APNG, stills | `MediaFile::frame_at`, `frame_number`, `frames` |
| Decode audio windows | AAC, Opus, Vorbis, MP1/2/3, FLAC, ALAC, AC-3, PCM in all the above plus WAV/AIFF/FLAC/MP3/Ogg files | `MediaFile::audio_window`, `audio_range` |
| Packet remux | MP4 <-> MOV <-> Matroska <-> WebM <-> fragmented MP4 (codec table in `remux` docs) | `remux`, `remux_to_vec`, `remux_file` |
| Segmenter | CMAF fMP4 init + media segments, HLS media and variant playlists | `segment`, `segment_with`, `hls_playlist`, `hls_variant` |
| Encode video | H.264 (CRF, CBR, VBR, two-pass, B-frames), ProRes (6 flavours), DNxHR (4), APV, Motion-JPEG into MP4/MOV | `Encoder`, `analyze_h264` |
| Encode audio | AAC-LC, PCM16, PCM float into MP4/MOV | `Encoder::push_audio` |
| Encode stills and animation | GIF, PNG, WAV | `encode_gif`, `encode_png`, `encode_wav` |
| Control | cancellation, deadline, progress | `Control`, `CancelToken`, `Progress` |
| Budgets | input size, whole-file reads, picture pixels, audio samples per request, batch frames, packet size and count, frame cache bytes | `Limits` |

Features: `container` (Demuxer, remux, segment: only `filmcraft-isobmff`, `filmcraft-matroska`,
`filmcraft-time`), `decode` (adds `filmcraft-codecs`, `-media`, `-frame`, `-color`), `encode` (adds
`filmcraft-export`, `-h264enc`, `image`). All are on by default.

Dependency tree size (`cargo tree -p <crate> -e normal --prefix none | sort -u | wc -l`, `(*)` repeats
removed), measured on this branch:

| crate / features | packages |
|---|---|
| `filmcraft-engine` (the editor engine, for scale) | 126 |
| `filmcraft-codecs` (what Bun Video depended on) | 94 |
| `filmcraft-lib`, default features (decode + container + encode) | 119 |
| `filmcraft-lib`, `decode` only | 95 |
| `filmcraft-lib`, `encode` only | 83 |
| `filmcraft-lib`, `container` only | 15 |

None of the `filmcraft-lib` trees contains egui, eframe, winit, wgpu, candle, cpal, rfd, speech, tts,
`filmcraft-gpu`, `filmcraft-engine` or `filmcraft-ui-egui` (`cargo tree -p filmcraft-lib | grep` finds 0).

## Guarantees

- **No panics on hostile input.** Every function returns `Result<_, Error>`. The test suites
  mutate (bit flips, truncation) MP4 and Matroska files through probe, packet reads, remux, segment,
  decode and audio and assert `catch_unwind` never trips. The crate denies `unwrap`, `expect`,
  `panic!`, `todo!`, `unreachable!` outside tests (`doc_fixtures` is the one documented exception: it
  builds files from literals).
- **Thread safety.** `Demuxer`, `MediaFile`, `Control`, `CancelToken`, `Limits` and the reports are
  `Send + Sync` (compile-time checks in the tests); `Encoder` is `Send`.
- **Bounded memory.** Allocations sized by the file are checked against `Limits` before they happen;
  the decoded-frame cache is capped by `set_frame_budget` / `Limits::frame_cache_bytes`.
- **Determinism.** The encoders are deterministic (same input, same bytes on every machine).
- **WASM.** The crate compiles for `wasm32-unknown-unknown` (`cargo xtask wasm` covers it as an L4
  crate); `FileSource` / `*_path` helpers are native only, and a deadline is only checked between calls
  there (no watchdog thread).

## Stability and semver

The workspace version (`0.7.x`) applies. Until 1.0:

- The **public items re-exported at the root of `filmcraft-lib`** are the API. Anything reached only
  through a dependency crate (`filmcraft_codecs::...`, `filmcraft_media::...`) is not covered, except
  the types the facade re-exports (`MediaInfo`, `VideoFrame`, `AudioBuffer`, `Tick`, `FrameRate`).
- Option and report structs and the enums `ErrorKind`, `ContainerKind`, `TrackKind`, `OutputContainer`,
  `VideoCodec`, `AudioCodec`, `RateControl`, `Stage`, `FrameCount`, `SegmentEvent` are
  `#[non_exhaustive]`: construct options with the `new` functions and set public fields; adding a
  field or variant is a minor release.
- `ErrorKind` is the stable error classification; messages are not.
- A minor release (`0.x`) may rename or remove an item after one release that deprecates it; a patch
  release only fixes bugs. Behaviour that changes output bytes (encoder improvements) is a minor release.
- `doc_fixtures` is hidden and unstable.

## Roadmap: not done, with the evidence

| Gap | Evidence | Why it is not in this change |
|---|---|---|
| HEVC and AV1 encoders | `crates/export/src/lib.rs` `Format::Hevc` / `Format::Av1`: "There is no built-in encoder: a platform hardware encoder registers one"; no `Encoder` type in `crates/hevc`, `crates/av1` | a new codec each |
| VP8 / VP9 / Opus / Vorbis / MP3 / FLAC-in-MP4 encoders | no encoder in `crates/vp8`, `vp9`, `opus` (decoders only); `FlacAudio` in `export` is crate-private | large; FLAC could be exposed with a small change in `export` |
| Matroska/WebM *encode* output | `Encoder` writes MP4/MOV; use `remux_to_vec` on its output for Matroska/WebM | the packet path exists, the encoder's pts bookkeeping for B-frames into `MkvWriter` is not wired |
| Exact frame index for AVI, MXF, MPEG-TS/PS, Ogg | their demuxers (`filmcraft-avi`, `-mxf`, `-mpegts`, `-ogg`) build sample tables, but `Probe::frame_count` returns `FrameCount::Estimated` for them | per-format adapters to `FrameIndex` |
| Remux of AVI/MXF/TS/Ogg sources and to those containers | `Demuxer` opens MP4/MOV/Matroska/WebM only; `filmcraft_mxf::write` is not wrapped | same adapters |
| E-AC-3, VP8, Vorbis, DNxHD into MP4; Matroska VFW codecs | `convert.rs` returns `Unsupported` | no sample-entry mapping implemented (E-AC-3 needs `dec3` from the bitstream) |
| DASH manifests | only HLS playlists exist (`hls_playlist`, `hls_variant`); CMAF segments are DASH-compatible | manifest writer |
| Subtitle track copy | `select_tracks` skips non-audio/video tracks | needs WebVTT/tx3g mapping |
| Hard memory cap on the shared decoder pool after first use | `set_frame_budget` returns `false` once the pool exists | pool resize |
| Native `TimeoutStream` watchdog on wasm | no threads on `wasm32` | platform limit |
