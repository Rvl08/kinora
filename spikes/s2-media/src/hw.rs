//! Thin FFI helpers for what ffmpeg-next does not wrap: hardware devices,
//! GPU -> system memory frame transfer, and raw codec-context fields.

use std::ptr;

use ffmpeg::ffi::*;
use ffmpeg::util::frame;
use ffmpeg_next as ffmpeg;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    D3d11va,
    Cuda,
}

impl Device {
    fn kind(self) -> AVHWDeviceType {
        match self {
            Device::D3d11va => AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA,
            Device::Cuda => AVHWDeviceType::AV_HWDEVICE_TYPE_CUDA,
        }
    }
    pub fn hw_pix_fmt(self) -> AVPixelFormat {
        match self {
            Device::D3d11va => AVPixelFormat::AV_PIX_FMT_D3D11,
            Device::Cuda => AVPixelFormat::AV_PIX_FMT_CUDA,
        }
    }
}

/// Attaches a new hardware device to an unopened decoder context.
///
/// # Safety
/// `ctx` must be a valid, not yet opened `AVCodecContext`.
pub unsafe fn attach_device(ctx: *mut AVCodecContext, device: Device) -> Result<(), String> {
    let mut dev: *mut AVBufferRef = ptr::null_mut();
    let r =
        unsafe { av_hwdevice_ctx_create(&mut dev, device.kind(), ptr::null(), ptr::null_mut(), 0) };
    if r < 0 {
        return Err(format!(
            "av_hwdevice_ctx_create({device:?}) failed: {}",
            ffmpeg::Error::from(r)
        ));
    }
    unsafe {
        (*ctx).hw_device_ctx = dev; // ownership moves to the context
        // Ask for the device's surface format explicitly (as the ffmpeg CLI does);
        // the default chooser silently fell back to software for CUDA.
        (*ctx).opaque = device.hw_pix_fmt() as i32 as isize as *mut std::ffi::c_void;
        (*ctx).get_format = Some(get_hw_format);
    }
    Ok(())
}

unsafe extern "C" fn get_hw_format(
    ctx: *mut AVCodecContext,
    fmts: *const AVPixelFormat,
) -> AVPixelFormat {
    unsafe {
        let want = (*ctx).opaque as isize as i32;
        let mut p = fmts;
        while *p != AVPixelFormat::AV_PIX_FMT_NONE {
            if *p as i32 == want {
                return *p;
            }
            p = p.add(1);
        }
        avcodec_default_get_format(ctx, fmts)
    }
}

/// A new reference to `f` (no pixel copy). ffmpeg-next's `clone()` deep-copies
/// the pixels, which also turns a GPU frame into an invalid one.
pub fn frame_ref(f: &frame::Video) -> frame::Video {
    let mut n = frame::Video::empty();
    unsafe {
        av_frame_ref(n.as_mut_ptr(), f.as_ptr());
    }
    n
}

pub fn is_hw_frame(f: &frame::Video) -> bool {
    unsafe {
        let fmt = (*f.as_ptr()).format;
        fmt == AVPixelFormat::AV_PIX_FMT_D3D11 as i32
            || fmt == AVPixelFormat::AV_PIX_FMT_CUDA as i32
    }
}

/// Copies a GPU frame into system memory (NV12 / P010), keeping its timestamp.
pub fn download(hw: &frame::Video) -> Result<frame::Video, String> {
    let mut sw = frame::Video::empty();
    unsafe {
        let r = av_hwframe_transfer_data(sw.as_mut_ptr(), hw.as_ptr(), 0);
        if r < 0 {
            return Err(format!(
                "av_hwframe_transfer_data: {}",
                ffmpeg::Error::from(r)
            ));
        }
        av_frame_copy_props(sw.as_mut_ptr(), hw.as_ptr());
    }
    Ok(sw)
}

/// Encoder settings that ffmpeg-next does not expose.
pub struct EncoderFields {
    pub width: i32,
    pub height: i32,
    pub pix_fmt: AVPixelFormat,
    pub time_base: AVRational,
    pub frame_rate: AVRational,
    pub bit_rate: i64,
    /// Set a VBV (max rate + buffer). Hardware encoders want it; SVT-AV1 rejects
    /// a max rate outside CRF mode.
    pub vbv: bool,
    pub gop: i32,
    pub max_b_frames: Option<i32>,
    pub global_header: bool,
    pub color: Option<(
        AVColorPrimaries,
        AVColorTransferCharacteristic,
        AVColorSpace,
        AVColorRange,
    )>,
    /// Zero-copy: share the decoder's hardware frames with the encoder.
    pub hw_frames_ctx: Option<*mut AVBufferRef>,
    pub threads: i32,
}

/// # Safety
/// `c` must be a valid, not yet opened `AVCodecContext`.
pub unsafe fn apply(c: *mut AVCodecContext, f: &EncoderFields) {
    unsafe {
        (*c).width = f.width;
        (*c).height = f.height;
        (*c).pix_fmt = f.pix_fmt;
        (*c).time_base = f.time_base;
        (*c).framerate = f.frame_rate;
        (*c).bit_rate = f.bit_rate;
        if f.vbv {
            (*c).rc_max_rate = f.bit_rate * 3 / 2;
            (*c).rc_buffer_size = (f.bit_rate * 2) as i32;
        }
        (*c).gop_size = f.gop;
        if let Some(b) = f.max_b_frames {
            (*c).max_b_frames = b;
        }
        (*c).thread_count = f.threads;
        if f.global_header {
            (*c).flags |= AV_CODEC_FLAG_GLOBAL_HEADER as i32;
        }
        if let Some((p, t, s, r)) = f.color {
            (*c).color_primaries = p;
            (*c).color_trc = t;
            (*c).colorspace = s;
            (*c).color_range = r;
        }
        if let Some(frames) = f.hw_frames_ctx {
            (*c).hw_frames_ctx = av_buffer_ref(frames);
        }
    }
}

pub fn pix_fmt_name(fmt: i32) -> String {
    unsafe {
        let p = av_get_pix_fmt_name(std::mem::transmute::<i32, AVPixelFormat>(fmt));
        if p.is_null() {
            format!("#{fmt}")
        } else {
            std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}
