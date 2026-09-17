//! MF 最后释放样本后归还纹理；WriteSample 返回不表示 GPU 已经消费完毕。
use std::sync::mpsc::SyncSender;
use windows::{
    Win32::{
        Foundation::E_NOTIMPL, Graphics::Direct3D11::ID3D11Texture2D, Media::MediaFoundation::*,
    },
    core::{Interface, Ref, implement},
};

#[implement(IMFAsyncCallback)]
struct Recycle {
    returned: SyncSender<ID3D11Texture2D>,
}
impl IMFAsyncCallback_Impl for Recycle_Impl {
    fn GetParameters(&self, _: *mut u32, _: *mut u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn Invoke(&self, result: Ref<'_, IMFAsyncResult>) -> windows::core::Result<()> {
        // SAFETY: MF 在样本引用归零后调用；state 是 SetAllocator 持有的纹理。
        let texture = unsafe { result.ok()?.GetState()? }.cast::<ID3D11Texture2D>()?;
        // 回调绝不阻塞 MF 工作线程；接收端停止后直接释放纹理。
        let _ = self.returned.try_send(texture);
        Ok(())
    }
}
pub(super) fn recycler(returned: SyncSender<ID3D11Texture2D>) -> IMFAsyncCallback {
    Recycle { returned }.into()
}
pub(super) fn sample(
    texture: &ID3D11Texture2D,
    pts: i64,
    duration: i64,
    callback: &IMFAsyncCallback,
) -> super::model::Result<IMFSample> {
    // SAFETY: 纹理由独占池槽提供，在 tracked sample 回调前不会再次写入。
    unsafe {
        let tracked =
            MFCreateTrackedSample().map_err(|source| super::model::VideoError::Stage {
                stage: "MFCreateTrackedSample",
                source,
            })?;
        let sample: IMFSample = tracked.cast()?;
        let buffer = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, texture, 0, false).map_err(
            |source| super::model::VideoError::Stage {
                stage: "MFCreateDXGISurfaceBuffer",
                source,
            },
        )?;
        buffer.SetCurrentLength(buffer.GetMaxLength()?)?;
        sample.AddBuffer(&buffer)?;
        sample.SetSampleTime(pts)?;
        sample.SetSampleDuration(duration)?;
        tracked.SetAllocator(callback, texture).map_err(|source| {
            super::model::VideoError::Stage {
                stage: "SetAllocator",
                source,
            }
        })?;
        Ok(sample)
    }
}
