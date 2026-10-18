# filmcraft-lib

FilmCraft as a library: pure-Rust probing, decoding, remuxing, segmenting and encoding of video and
audio, with no FFmpeg, no `unsafe` and none of the editor (no UI, GPU, speech or text-to-speech
crates).

```rust
use filmcraft_lib::{Control, Limits, MediaFile, OutputContainer, RemuxOptions, Tick, probe_path, remux_file, Demuxer};

let ctl = Control::default();
let limits = Limits::default();

// what is in the file, with an exact frame count for MP4/MOV/Matroska/WebM
let p = probe_path("in.mkv".as_ref(), &limits, &ctl)?;
println!("{:?} frames", p.frame_count());

// seek + decode
let file = MediaFile::open_path("in.mkv".as_ref(), &limits, &ctl)?;
let frame = file.frame_at(Tick::from_seconds_f64(2.0), &ctl)?;
let rgba = frame.to_rgba8();

// copy the packets into MP4 without re-encoding
let input = Demuxer::open_path("in.mkv".as_ref(), &limits)?;
remux_file(&input, "out.mp4".as_ref(), &RemuxOptions::new(OutputContainer::Mp4), &ctl)?;
```

See the crate docs (`cargo doc -p filmcraft-lib --open`) and [`LIBRARY.md`](../../LIBRARY.md) for the
full capability table, the stability policy and the roadmap.

## Modules at a glance

| Module / item | Needs feature | Does |
|---|---|---|
| `Demuxer`, `FrameIndex`, `TrackInfo`, `Packet` | `container` | open MP4/MOV/Matroska/WebM, exact per-track packet table, packet reads |
| `remux`, `remux_to_vec`, `remux_file`, `RemuxOptions` | `container` | MP4 <-> MOV <-> Matroska <-> WebM <-> fMP4 without re-encoding |
| `segment`, `segment_with`, `hls_playlist` | `container` | CMAF segments and HLS playlists |
| `MediaFile`, `probe*` | `decode` | seek/decode video, audio windows, probe |
| `Encoder`, `encode_gif`, `encode_png`, `encode_wav`, `analyze_h264` | `encode` | encoders |
| `Control`, `CancelToken`, `Limits`, `Error`, `ErrorKind` | always | cancellation, deadline, progress, caps, errors |

## Integrating from another workspace

```toml
[dependencies]
filmcraft-lib = { git = "https://github.com/aphrody-labs/filmcraft", rev = "<commit>", default-features = false, features = ["decode", "container"] }
```

Pin a `rev`: the library follows the workspace version and is `0.x`.
