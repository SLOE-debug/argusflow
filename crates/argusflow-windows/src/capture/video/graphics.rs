//! 专用视频设备和 GPU 上的 BGRA→NV12 转换，不创建 CPU staging 图像。
use super::model::{Result, required};
use std::mem::ManuallyDrop;
use windows::{
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            Dxgi::{Common::*, IDXGIAdapter1},
        },
    },
    core::Interface,
};

pub(super) struct Graphics {
    pub device: ID3D11Device,
    context: ID3D11DeviceContext,
    video: ID3D11VideoDevice,
    processor: ID3D11VideoProcessor,
    enumeration: ID3D11VideoProcessorEnumerator,
    video_context: ID3D11VideoContext1,
    width: u32,
    height: u32,
}
impl Graphics {
    pub fn new(adapter: &IDXGIAdapter1, width: u32, height: u32, fps: u32) -> Result<Self> {
        // SAFETY: 独立硬件设备，多线程保护使 MF 和采集可共享设备。
        unsafe {
            let mut device = None;
            let mut context = None;
            D3D11CreateDevice(
                adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
            let device = required(device, "D3D11 device")?;
            let context = required(context, "D3D11 context")?;
            let protection: ID3D11Multithread = context.cast()?;
            let _ = protection.SetMultithreadProtected(true);
            let video: ID3D11VideoDevice = device.cast()?;
            let video_context: ID3D11VideoContext1 = context.cast()?;
            let rate = DXGI_RATIONAL {
                Numerator: fps,
                Denominator: 1,
            };
            let enumeration =
                video.CreateVideoProcessorEnumerator(&D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
                    InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                    InputFrameRate: rate,
                    InputWidth: width,
                    InputHeight: height,
                    OutputFrameRate: rate,
                    OutputWidth: width,
                    OutputHeight: height,
                    Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
                })?;
            let processor = video.CreateVideoProcessor(&enumeration, 0)?;
            // SDR 桌面为 full RGB，编码明确使用 BT.709 limited NV12。
            video_context.VideoProcessorSetStreamColorSpace1(
                &processor,
                0,
                DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709,
            );
            video_context.VideoProcessorSetOutputColorSpace1(
                &processor,
                DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
            );
            video_context.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
            Ok(Self {
                device,
                context,
                video,
                processor,
                enumeration,
                video_context,
                width,
                height,
            })
        }
    }
    pub fn texture(&self) -> Result<ID3D11Texture2D> {
        let mut texture = None;
        // SAFETY: pool texture 为单层 NV12 render target，永不被 CPU 映射。
        unsafe {
            self.device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: self.width,
                    Height: self.height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_NV12,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut texture),
            )?;
        }
        required(texture, "NV12 texture")
    }
    pub fn convert(&self, input: &ID3D11Texture2D, output: &ID3D11Texture2D) -> Result<()> {
        // SAFETY: 输入在 DXGI lease 内有效，输出只由采集线程持有。
        unsafe {
            let mut source = None;
            self.video.CreateVideoProcessorInputView(
                input,
                &self.enumeration,
                &D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
                    ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
                    ..Default::default()
                },
                Some(&mut source),
            )?;
            let mut target = None;
            self.video.CreateVideoProcessorOutputView(
                output,
                &self.enumeration,
                &D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                    ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                    ..Default::default()
                },
                Some(&mut target),
            )?;
            let target = required(target, "video output view")?;
            let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
                Enable: true.into(),
                pInputSurface: ManuallyDrop::new(source),
                ..Default::default()
            };
            let result = self.video_context.VideoProcessorBlt(
                &self.processor,
                &target,
                0,
                std::slice::from_ref(&stream),
            );
            ManuallyDrop::drop(&mut stream.pInputSurface);
            result?;
            // 先提交转换命令，再把纹理交给 MF；同一设备保证 GPU 命令依赖。
            self.context.Flush();
        }
        Ok(())
    }
    pub fn copy(&self, input: &ID3D11Texture2D, output: &ID3D11Texture2D) {
        // SAFETY: 两个纹理来自同一设备且具有相同NV12描述，只在采集线程复制。
        unsafe {
            self.context.CopyResource(output, input);
            self.context.Flush();
        }
    }
}
