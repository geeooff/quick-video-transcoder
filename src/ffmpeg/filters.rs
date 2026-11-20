//! The video filter graph shared by the preview, the size estimate and the encode, so what the preview
//! shows is what the encode writes.
//!
//! Two engines:
//! - **libplacebo** on a Vulkan device (the default when [`super::hardware::Capabilities::placebo`]):
//!   scaling, frame rate and tone mapping on the GPU, with dynamic peak detection (NVIDIA captures carry no
//!   mastering metadata) and perceptual gamut mapping. The video is decoded by Vulkan when it can
//!   ([`super::hardware::vulkan_decodes`]), else decoded on the CPU and uploaded. Measured on an RTX 4090:
//!   4K60 AV1 HDR to 720p30 SDR at 4.4x real time.
//! - **software**: `scale`, and for HDR the `zscale` + `tonemap` chain, on the CPU.
//!
//! Color is always explicit: the source's range is read from the file (NVIDIA HDR captures are full
//! range), an untagged source is declared BT.709 limited range, and the output is tagged.

use super::encoders::Depth;
use super::hardware::VULKAN_DEVICE;
use super::probe::{Dynamic, MediaInfo};
use crate::plan::{FrameRate, Size};
use crate::profile::ToneMapping;

/// How the filters run for one video.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Engine {
    /// libplacebo on Vulkan, else the software filters.
    pub placebo: bool,
    /// Vulkan decodes the video (only with `placebo`); else the CPU or `-hwaccel auto` does.
    pub vulkan_decode: bool,
}

impl Engine {
    /// The software engine.
    pub const SOFTWARE: Self = Self {
        placebo: false,
        vulkan_decode: false,
    };

    /// Name for the log and the status line.
    pub fn label(self) -> &'static str {
        match (self.placebo, self.vulkan_decode) {
            (true, true) => "Vulkan decode + libplacebo",
            (true, false) => "libplacebo",
            (false, _) => "software filters",
        }
    }
}

/// The color of the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputColor {
    /// 8-bit BT.709 SDR, limited range. HDR sources are tone mapped.
    Sdr,
    /// 10-bit BT.2020 HDR, same transfer as the source (PQ or HLG), limited range.
    Hdr(Dynamic),
}

impl OutputColor {
    /// The output color of `media` for a profile that may (`sdr_only == false`) or may not keep HDR with
    /// a codec that may carry it.
    pub fn choose(media: &MediaInfo, sdr_only: bool, codec_supports_hdr: bool) -> Self {
        if media.dynamic.is_hdr() && !sdr_only && codec_supports_hdr {
            Self::Hdr(media.dynamic)
        } else {
            Self::Sdr
        }
    }

    /// The bit depth the encoder receives.
    pub fn depth(self) -> Depth {
        match self {
            Self::Sdr => Depth::Sdr8,
            Self::Hdr(_) => Depth::Hdr10,
        }
    }

    /// Color tags written in the output (after the encoder options).
    pub fn tag_args(self) -> Vec<String> {
        let (matrix, transfer, primaries) = match self {
            Self::Sdr => ("bt709", "bt709", "bt709"),
            Self::Hdr(dynamic) => ("bt2020nc", transfer_name(dynamic), "bt2020"),
        };
        [
            "-colorspace:v",
            matrix,
            "-color_trc:v",
            transfer,
            "-color_primaries:v",
            primaries,
            "-color_range:v",
            "tv",
        ]
        .map(String::from)
        .to_vec()
    }
}

fn transfer_name(dynamic: Dynamic) -> &'static str {
    match dynamic {
        Dynamic::Sdr => "bt709",
        Dynamic::Pq => "smpte2084",
        Dynamic::Hlg => "arib-std-b67",
    }
}

/// Where the filtered frames go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sink {
    /// An encoder, writing `color`.
    Encoder(OutputColor),
    /// The preview: RGBA in full range for the screen. An HDR output is shown tone mapped, since the
    /// preview is drawn in SDR.
    Preview,
}

/// What the filter graph needs to know about the output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphSpec {
    /// Output picture size.
    pub size: Size,
    /// Output frame rate, `None` to keep every source frame (the preview).
    pub rate: Option<FrameRate>,
    /// HDR to SDR curve.
    pub tone: ToneMapping,
    /// Destination of the frames.
    pub sink: Sink,
}

/// The options before `-i` and the `-vf` value for one video.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoGraph {
    /// Device and decoder options, placed before `-ss` and `-i`.
    pub input_args: Vec<String>,
    /// The `-vf` filter chain.
    pub filter: String,
}

/// Builds the graph turning `media` into `spec` with `engine`.
pub fn graph(engine: Engine, media: &MediaInfo, spec: GraphSpec) -> VideoGraph {
    if engine.placebo {
        placebo_graph(engine.vulkan_decode, media, spec)
    } else {
        software_graph(media, spec)
    }
}

/// `setparams` declaring BT.709 limited range on a source with no color tag at all, so nothing guesses.
fn untagged_fix(media: &MediaInfo) -> Option<&'static str> {
    (media.untagged && !media.dynamic.is_hdr())
        .then_some("setparams=colorspace=bt709:color_trc=bt709:color_primaries=bt709:range=tv")
}

fn placebo_graph(vulkan_decode: bool, media: &MediaInfo, spec: GraphSpec) -> VideoGraph {
    let mut input_args: Vec<String> = VULKAN_DEVICE.map(String::from).to_vec();
    if vulkan_decode {
        input_args
            .extend(["-hwaccel", "vulkan", "-hwaccel_output_format", "vulkan"].map(String::from));
    } else {
        input_args.extend(["-hwaccel", "auto"].map(String::from));
    }
    let mut chain: Vec<String> = Vec::new();
    // FFmpeg's `fps` filter, not libplacebo's `fps` option: after a seek, libplacebo's own timeline starts
    // before the first source frame and outputs one blank (transparent black) frame first. `fps` works on
    // Vulkan frames too, and libplacebo then processes only the frames kept.
    if let Some(rate) = spec.rate {
        chain.push(format!("fps={}", rate.arg()));
    }
    if let Some(fix) = untagged_fix(media) {
        chain.push(fix.to_owned());
    }
    if !vulkan_decode {
        chain.push("hwupload".to_owned());
    }
    let mut options = vec![
        format!("w={}", spec.size.width),
        format!("h={}", spec.size.height),
    ];
    // Orthogonal Lanczos: sharp downscaling for game footage, cheap enough for the preview.
    options.push("downscaler=lanczos".to_owned());
    let (output, format) = match spec.sink {
        Sink::Encoder(OutputColor::Sdr) => (
            "colorspace=bt709:color_primaries=bt709:color_trc=bt709:range=tv".to_owned(),
            "nv12",
        ),
        Sink::Encoder(OutputColor::Hdr(dynamic)) => (
            format!(
                "colorspace=bt2020nc:color_primaries=bt2020:color_trc={}:range=tv",
                transfer_name(dynamic)
            ),
            "p010le",
        ),
        Sink::Preview => (
            "color_primaries=bt709:color_trc=bt709:range=pc".to_owned(),
            "rgba",
        ),
    };
    options.push(output);
    if media.dynamic.is_hdr() {
        options.push(format!("tonemapping={}", spec.tone.placebo()));
    }
    options.push(format!("format={format}"));
    chain.push(format!("libplacebo={}", options.join(":")));
    chain.push(format!("hwdownload,format={format}"));
    VideoGraph {
        input_args,
        filter: chain.join(","),
    }
}

fn software_graph(media: &MediaInfo, spec: GraphSpec) -> VideoGraph {
    let mut chain: Vec<String> = Vec::new();
    if let Some(rate) = spec.rate {
        chain.push(format!("fps={}", rate.arg()));
    }
    let Size { width, height } = spec.size;
    let source_range = if media.full_range { "full" } else { "limited" };
    let tone_mapped =
        media.dynamic.is_hdr() && spec.sink != Sink::Encoder(OutputColor::Hdr(media.dynamic));
    if tone_mapped {
        // Linear light at 100 nits for SDR white, tone map in BT.709 primaries, back to BT.709.
        chain.push(format!(
            "zscale=w={width}:h={height}:filter=lanczos:tin={}:min=bt2020nc:pin=bt2020:rin={source_range}:\
             t=linear:npl=100",
            transfer_name(media.dynamic)
        ));
        chain.push("format=gbrpf32le".to_owned());
        chain.push("zscale=p=bt709".to_owned());
        chain.push(format!("tonemap=tonemap={}:desat=0", spec.tone.software()));
        match spec.sink {
            Sink::Preview => chain.push("zscale=t=bt709,format=rgba".to_owned()),
            Sink::Encoder(_) => {
                chain.push("zscale=t=bt709:m=bt709:r=limited".to_owned());
                chain.push("format=yuv420p".to_owned());
            }
        }
    } else {
        if let Some(fix) = untagged_fix(media) {
            chain.push(fix.to_owned());
        }
        let (flags, out_range, format) = match spec.sink {
            Sink::Encoder(OutputColor::Sdr) => ("lanczos", "tv", "yuv420p"),
            Sink::Encoder(OutputColor::Hdr(_)) => ("lanczos", "tv", "yuv420p10le"),
            Sink::Preview => ("bilinear", "pc", "rgba"),
        };
        let matrix = if media.dynamic.is_hdr() {
            "bt2020"
        } else {
            "bt709"
        };
        let out_matrix = match spec.sink {
            Sink::Encoder(OutputColor::Hdr(_)) => "bt2020",
            _ => "bt709",
        };
        let in_range = if media.full_range { "pc" } else { "tv" };
        chain.push(format!(
            "scale={width}:{height}:flags={flags}:in_color_matrix={matrix}:in_range={in_range}:\
             out_color_matrix={out_matrix}:out_range={out_range}"
        ));
        chain.push(format!("format={format}"));
    }
    VideoGraph {
        input_args: ["-hwaccel", "auto"].map(String::from).to_vec(),
        filter: chain.join(","),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg::probe::parse_probe_json;

    fn hdr_capture() -> MediaInfo {
        parse_probe_json(
            r#"{"streams": [{"codec_type": "video", "codec_name": "av1", "width": 3840,
                "height": 2160, "r_frame_rate": "60/1", "color_space": "bt2020nc",
                "color_range": "pc", "color_transfer": "smpte2084", "color_primaries": "bt2020"}]}"#,
        )
        .unwrap()
    }

    fn untagged_capture() -> MediaInfo {
        parse_probe_json(
            r#"{"streams": [{"codec_type": "video", "codec_name": "h264", "width": 1920,
                "height": 1080, "r_frame_rate": "60000/1001"}]}"#,
        )
        .unwrap()
    }

    const HD: Size = Size {
        width: 1280,
        height: 720,
    };

    fn encode(color: OutputColor) -> GraphSpec {
        GraphSpec {
            size: HD,
            rate: Some(FrameRate::new(30, 1)),
            tone: ToneMapping::Auto,
            sink: Sink::Encoder(color),
        }
    }

    const VULKAN: Engine = Engine {
        placebo: true,
        vulkan_decode: true,
    };

    #[test]
    fn tone_maps_hdr_on_the_gpu() {
        let g = graph(VULKAN, &hdr_capture(), encode(OutputColor::Sdr));
        assert_eq!(
            g.input_args.join(" "),
            "-init_hw_device vulkan=vk -filter_hw_device vk -hwaccel vulkan -hwaccel_output_format vulkan"
        );
        assert_eq!(
            g.filter,
            "fps=30,libplacebo=w=1280:h=720:downscaler=lanczos:\
             colorspace=bt709:color_primaries=bt709:color_trc=bt709:range=tv:tonemapping=auto:\
             format=nv12,hwdownload,format=nv12"
        );
    }

    #[test]
    fn uploads_when_vulkan_cannot_decode_and_declares_untagged_sources() {
        let engine = Engine {
            placebo: true,
            vulkan_decode: false,
        };
        let g = graph(engine, &untagged_capture(), encode(OutputColor::Sdr));
        assert!(g.input_args.ends_with(&["-hwaccel".into(), "auto".into()]));
        assert!(
            g.filter.starts_with(
                "fps=30,setparams=colorspace=bt709:color_trc=bt709:color_primaries=bt709:range=tv,hwupload,libplacebo="
            ),
            "{}",
            g.filter
        );
        assert!(!g.filter.contains("tonemapping"), "{}", g.filter);
    }

    #[test]
    fn keeps_hdr_in_ten_bits_when_the_profile_allows_it() {
        let media = hdr_capture();
        let color = OutputColor::choose(&media, false, true);
        assert_eq!(color, OutputColor::Hdr(Dynamic::Pq));
        assert_eq!(OutputColor::choose(&media, true, true), OutputColor::Sdr);
        assert_eq!(OutputColor::choose(&media, false, false), OutputColor::Sdr);
        let g = graph(VULKAN, &media, encode(color));
        assert!(
            g.filter.contains("color_trc=smpte2084:range=tv"),
            "{}",
            g.filter
        );
        assert!(
            g.filter.ends_with("format=p010le,hwdownload,format=p010le"),
            "{}",
            g.filter
        );
        assert_eq!(
            color.tag_args().join(" "),
            "-colorspace:v bt2020nc -color_trc:v smpte2084 -color_primaries:v bt2020 -color_range:v tv"
        );
    }

    #[test]
    fn previews_in_rgba_without_changing_the_frame_rate() {
        let spec = GraphSpec {
            rate: None,
            sink: Sink::Preview,
            ..encode(OutputColor::Sdr)
        };
        let g = graph(VULKAN, &hdr_capture(), spec);
        assert!(!g.filter.contains("fps="), "{}", g.filter);
        assert!(
            g.filter.contains("range=pc:tonemapping=auto:format=rgba"),
            "{}",
            g.filter
        );
        let s = graph(Engine::SOFTWARE, &hdr_capture(), spec);
        assert!(
            s.filter
                .ends_with("tonemap=tonemap=hable:desat=0,zscale=t=bt709,format=rgba"),
            "{}",
            s.filter
        );
    }

    #[test]
    fn tone_maps_on_the_cpu_from_the_source_range() {
        let g = graph(Engine::SOFTWARE, &hdr_capture(), encode(OutputColor::Sdr));
        assert_eq!(g.input_args, ["-hwaccel", "auto"]);
        assert_eq!(
            g.filter,
            "fps=30,zscale=w=1280:h=720:filter=lanczos:tin=smpte2084:min=bt2020nc:pin=bt2020:rin=full:\
             t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,\
             zscale=t=bt709:m=bt709:r=limited,format=yuv420p"
        );
        let sdr = graph(
            Engine::SOFTWARE,
            &untagged_capture(),
            encode(OutputColor::Sdr),
        );
        assert!(
            sdr.filter
                .contains("scale=1280:720:flags=lanczos:in_color_matrix=bt709:in_range=tv"),
            "{}",
            sdr.filter
        );
    }
}
