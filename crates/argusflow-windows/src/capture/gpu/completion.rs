//! 基线发布前的 GPU 完成栅栏，不能仅凭命令已提交就推进健康水位。
use super::{Graphics, failure, invalid};
use argusflow_capture_contracts::{CaptureResult, Reservation};
use std::time::Instant;
use windows::{Win32::Graphics::Direct3D11::*, core::BOOL};
pub(in crate::capture) struct Completion {
    query: ID3D11Query,
    _bytes: Reservation,
    pub started: Instant,
}
impl Completion {
    pub fn submit(graphics: &Graphics) -> CaptureResult<Self> {
        let bytes = graphics.budget.reserve(64)?;
        let mut query = None;
        unsafe {
            graphics.device.CreateQuery(
                &D3D11_QUERY_DESC {
                    Query: D3D11_QUERY_EVENT,
                    MiscFlags: 0,
                },
                Some(&mut query),
            )
        }
        .map_err(failure)?;
        let query = query.ok_or_else(|| invalid("missing completion query"))?;
        // SAFETY: EVENT query 只需 End，同设备同线程 context。
        unsafe { graphics.context.End(&query) };
        Ok(Self {
            query,
            _bytes: bytes,
            started: Instant::now(),
        })
    }
    pub fn ready(&self, graphics: &Graphics) -> CaptureResult<bool> {
        let mut done = BOOL(0);
        // GetData 的 S_FALSE 在 windows-rs 中也映射为 Ok；必须同时核对 BOOL。
        unsafe {
            graphics.context.GetData(
                &self.query,
                Some((&mut done as *mut BOOL).cast()),
                std::mem::size_of::<BOOL>() as u32,
                D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
            )
        }
        .map_err(failure)?;
        Ok(done.as_bool())
    }
}
