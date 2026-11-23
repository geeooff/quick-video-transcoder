//! The FFmpeg encoder of every codec / backend pair, and its two rate controls:
//!
//! - constant QP, for the size estimate's samples: the size then depends only on the picture, which is
//!   what the estimate measures;
//! - a target bitrate (VBR, two passes inside NVENC), for the encode, so the file lands at the size the
//!   estimate chose.
//!
//! Qualities are given on the H.264/H.265 QP scale (0–51, lower is better) everywhere in the application;
//! [`EncoderSpec::qp`] converts them for AV1 encoders whose scale is 0–255.

use serde::{Deserialize, Serialize};

/// A video codec the application can write.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VideoCodec {
    /// H.265 / HEVC: the default, played everywhere chat apps run.
    #[default]
    H265,
    /// H.264 / AVC: the most compatible, about 30 % larger for the same picture.
    H264,
    /// AV1: the most efficient, but not every phone decodes it in hardware.
    Av1,
}

impl VideoCodec {
    /// Every codec, in menu order.
    pub const ALL: [Self; 3] = [Self::H265, Self::H264, Self::Av1];

    /// Name shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::H265 => "H.265",
            Self::H264 => "H.264",
            Self::Av1 => "AV1",
        }
    }

    /// `true` when the codec can carry 10-bit HDR.
    pub fn supports_hdr(self) -> bool {
        self != Self::H264
    }
}

/// Who does the encoding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EncoderBackend {
    /// NVIDIA NVENC.
    #[default]
    Nvenc,
    /// Intel Quick Sync.
    Qsv,
    /// AMD AMF.
    Amf,
    /// The CPU (x265, x264, SVT-AV1).
    Software,
}

impl EncoderBackend {
    /// Every backend, best first.
    pub const ALL: [Self; 4] = [Self::Nvenc, Self::Qsv, Self::Amf, Self::Software];

    /// Name shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Nvenc => "NVENC",
            Self::Qsv => "Quick Sync",
            Self::Amf => "AMF",
            Self::Software => "CPU",
        }
    }
}

/// One FFmpeg encoder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncoderSpec {
    /// The codec it writes.
    pub codec: VideoCodec,
    /// Who runs it.
    pub backend: EncoderBackend,
    /// FFmpeg encoder name (`-c:v`).
    pub name: &'static str,
    /// Largest QP of its own scale (51, or 255 for NVENC and AMF AV1).
    pub qp_max: u32,
}

const fn spec(
    codec: VideoCodec,
    backend: EncoderBackend,
    name: &'static str,
    qp_max: u32,
) -> EncoderSpec {
    EncoderSpec {
        codec,
        backend,
        name,
        qp_max,
    }
}

static ENCODERS: [EncoderSpec; 12] = {
    use EncoderBackend::*;
    use VideoCodec::*;
    [
        spec(H265, Nvenc, "hevc_nvenc", 51),
        spec(H265, Qsv, "hevc_qsv", 51),
        spec(H265, Amf, "hevc_amf", 51),
        spec(H265, Software, "libx265", 51),
        spec(H264, Nvenc, "h264_nvenc", 51),
        spec(H264, Qsv, "h264_qsv", 51),
        spec(H264, Amf, "h264_amf", 51),
        spec(H264, Software, "libx264", 51),
        spec(Av1, Nvenc, "av1_nvenc", 255),
        spec(Av1, Qsv, "av1_qsv", 255),
        spec(Av1, Amf, "av1_amf", 255),
        spec(Av1, Software, "libsvtav1", 63),
    ]
};

/// Every supported encoder.
pub fn all() -> &'static [EncoderSpec] {
    &ENCODERS
}

/// The encoder of a codec / backend pair.
pub fn find(codec: VideoCodec, backend: EncoderBackend) -> &'static EncoderSpec {
    all()
        .iter()
        .find(|s| s.codec == codec && s.backend == backend)
        .expect("every codec has every backend")
}

/// Whether a picture is 8-bit SDR or 10-bit HDR when it reaches the encoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    /// 8-bit 4:2:0 (`nv12`).
    Sdr8,
    /// 10-bit 4:2:0 (`p010le`), HDR kept.
    Hdr10,
}

impl EncoderSpec {
    /// A quality on the H.265 QP scale converted to this encoder's scale.
    pub fn qp(&self, h265_qp: f64) -> u32 {
        let qp = h265_qp.clamp(0.0, 51.0) * f64::from(self.qp_max) / 51.0;
        qp.round() as u32
    }

    /// Options after `-c:v <name>` common to both rate controls: speed preset, keyframe interval, pixel
    /// format and profile.
    fn common_args(&self, gop: u32, depth: Depth) -> Vec<String> {
        let mut args: Vec<String> = vec!["-g:v".into(), gop.to_string()];
        match self.backend {
            EncoderBackend::Nvenc => {
                args.extend(["-preset:v", "p7", "-tune:v", "hq"].map(String::from));
                args.extend(["-spatial-aq:v", "1", "-temporal-aq:v", "1"].map(String::from));
                args.extend(["-bf:v", "3", "-b_ref_mode:v", "middle"].map(String::from));
                args.extend(["-rc-lookahead:v", "32"].map(String::from));
            }
            EncoderBackend::Qsv => {
                args.extend(
                    ["-preset:v", "veryslow", "-look_ahead_depth:v", "32"].map(String::from),
                );
            }
            EncoderBackend::Amf => {
                args.extend(["-quality:v", "quality"].map(String::from));
            }
            EncoderBackend::Software => match self.codec {
                VideoCodec::Av1 => args.extend(["-preset:v", "6"].map(String::from)),
                VideoCodec::H264 | VideoCodec::H265 => {
                    args.extend(["-preset:v", "slow"].map(String::from));
                }
            },
        }
        let pix_fmt = match (depth, self.backend) {
            (Depth::Sdr8, EncoderBackend::Software) => "yuv420p",
            (Depth::Hdr10, EncoderBackend::Software) => "yuv420p10le",
            (Depth::Sdr8, _) => "nv12",
            (Depth::Hdr10, _) => "p010le",
        };
        args.extend(["-pix_fmt:v".into(), pix_fmt.into()]);
        if self.codec == VideoCodec::H265 {
            // Apple players only accept H.265 in MP4 under the `hvc1` tag (FFmpeg writes `hev1`).
            args.extend(["-tag:v", "hvc1"].map(String::from));
            if depth == Depth::Hdr10 {
                args.extend(["-profile:v", "main10"].map(String::from));
            }
        }
        args
    }

    /// Constant QP (`h265_qp` on the H.265 scale), for the size estimate.
    pub fn constant_qp_args(&self, h265_qp: f64, gop: u32, depth: Depth) -> Vec<String> {
        let q = self.qp(h265_qp).to_string();
        let mut args = vec!["-c:v".to_owned(), self.name.to_owned()];
        args.extend(self.common_args(gop, depth));
        match self.backend {
            EncoderBackend::Nvenc => {
                args.extend(["-rc:v".into(), "constqp".into(), "-qp:v".into(), q]);
            }
            EncoderBackend::Qsv => {
                args.extend(["-q:v".into(), q]);
            }
            EncoderBackend::Amf => {
                args.extend(["-rc:v".into(), "cqp".into()]);
                for frame in ["-qp_i:v", "-qp_p:v", "-qp_b:v"] {
                    args.extend([frame.to_owned(), q.clone()]);
                }
            }
            EncoderBackend::Software => match self.codec {
                VideoCodec::H264 | VideoCodec::H265 => args.extend(["-qp:v".into(), q]),
                VideoCodec::Av1 => args.extend(["-qp:v".into(), q]),
            },
        }
        args
    }

    /// A target bitrate in bit/s, for the encode. The peak is capped at twice the target over a two
    /// second buffer, which keeps a short burst of motion sharp without letting the average drift.
    pub fn bitrate_args(&self, bps: f64, gop: u32, depth: Depth) -> Vec<String> {
        let rate = |factor: f64| format!("{}", (bps * factor).round() as u64);
        let mut args = vec!["-c:v".to_owned(), self.name.to_owned()];
        args.extend(self.common_args(gop, depth));
        match self.backend {
            EncoderBackend::Nvenc => {
                args.extend(["-rc:v", "vbr", "-multipass:v", "fullres"].map(String::from));
            }
            EncoderBackend::Amf => {
                args.extend(["-rc:v", "vbr_peak"].map(String::from));
            }
            EncoderBackend::Qsv | EncoderBackend::Software => {}
        }
        args.extend(["-b:v".into(), rate(1.0)]);
        // SVT-AV1 refuses a peak rate outside its CRF mode ("Max Bitrate only supported with CRF mode"):
        // its VBR keeps the average alone.
        if !(self.backend == EncoderBackend::Software && self.codec == VideoCodec::Av1) {
            args.extend([
                "-maxrate:v".into(),
                rate(2.0),
                "-bufsize:v".into(),
                rate(4.0),
            ]);
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pair_has_an_encoder() {
        for codec in VideoCodec::ALL {
            for backend in EncoderBackend::ALL {
                let spec = find(codec, backend);
                assert_eq!((spec.codec, spec.backend), (codec, backend));
            }
        }
    }

    #[test]
    fn converts_the_quality_scale_for_av1() {
        assert_eq!(find(VideoCodec::H265, EncoderBackend::Nvenc).qp(26.0), 26);
        assert_eq!(find(VideoCodec::Av1, EncoderBackend::Nvenc).qp(26.0), 130);
        assert_eq!(find(VideoCodec::Av1, EncoderBackend::Software).qp(51.0), 63);
        assert_eq!(find(VideoCodec::H264, EncoderBackend::Nvenc).qp(80.0), 51);
    }

    #[test]
    fn nvenc_encodes_two_pass_vbr_with_the_historical_options() {
        let spec = find(VideoCodec::H265, EncoderBackend::Nvenc);
        let args = spec.bitrate_args(2_500_000.0, 120, Depth::Sdr8).join(" ");
        assert_eq!(
            args,
            "-c:v hevc_nvenc -g:v 120 -preset:v p7 -tune:v hq -spatial-aq:v 1 -temporal-aq:v 1 \
             -bf:v 3 -b_ref_mode:v middle -rc-lookahead:v 32 -pix_fmt:v nv12 -tag:v hvc1 \
             -rc:v vbr -multipass:v fullres -b:v 2500000 -maxrate:v 5000000 -bufsize:v 10000000"
        );
        let qp = spec.constant_qp_args(27.4, 60, Depth::Hdr10).join(" ");
        assert!(qp.ends_with("-rc:v constqp -qp:v 27"), "{qp}");
        assert!(
            qp.contains("-pix_fmt:v p010le -tag:v hvc1 -profile:v main10"),
            "{qp}"
        );
    }

    #[test]
    fn svt_av1_gets_no_peak_rate() {
        let svt = find(VideoCodec::Av1, EncoderBackend::Software)
            .bitrate_args(1e6, 60, Depth::Sdr8)
            .join(" ");
        assert!(svt.ends_with("-b:v 1000000"), "{svt}");
        let x265 = find(VideoCodec::H265, EncoderBackend::Software)
            .bitrate_args(1e6, 60, Depth::Sdr8)
            .join(" ");
        assert!(
            x265.ends_with("-maxrate:v 2000000 -bufsize:v 4000000"),
            "{x265}"
        );
    }
}
