//! 有界任务通道及可复用解码器；许可随真实工作结束释放。
use super::{
    decoder::Decoder,
    review::{VideoFrame, read},
};
use std::{
    path::PathBuf,
    sync::{OnceLock, mpsc},
    time::Duration,
};
use tokio::sync::{SemaphorePermit, oneshot};

struct Request {
    directory: PathBuf,
    qpc: String,
    direction: i8,
    result_anchor: Option<i64>,
    settle_after: Option<i64>,
    reply: oneshot::Sender<Result<VideoFrame, String>>,
    _permit: SemaphorePermit<'static>,
}
pub(super) fn submit(
    directory: PathBuf,
    qpc: String,
    direction: i8,
    result_anchor: Option<i64>,
    settle_after: Option<i64>,
    permit: SemaphorePermit<'static>,
) -> Result<oneshot::Receiver<Result<VideoFrame, String>>, String> {
    static WORKER: OnceLock<Result<mpsc::SyncSender<Request>, String>> = OnceLock::new();
    let sender = WORKER
        .get_or_init(|| {
            let (sender, receiver) = mpsc::sync_channel::<Request>(1);
            std::thread::Builder::new()
                .name("video-review".into())
                .spawn(move || {
                    let mut decoder = Decoder::default();
                    let mut indices = super::index_cache::IndexCache::default();
                    loop {
                        match receiver.recv_timeout(Duration::from_secs(30)) {
                            Ok(request) => {
                                let result = read(
                                    request.directory,
                                    request.qpc,
                                    request.direction,
                                    request.result_anchor,
                                    request.settle_after,
                                    &mut decoder,
                                    &mut indices,
                                );
                                let _ = request.reply.send(result);
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                decoder = Decoder::default();
                                indices = super::index_cache::IndexCache::default();
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
            Ok(sender)
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let (reply, receiver) = oneshot::channel();
    sender
        .try_send(Request {
            directory,
            qpc,
            direction,
            result_anchor,
            settle_after,
            reply,
            _permit: permit,
        })
        .map_err(|e| e.to_string())?;
    Ok(receiver)
}
