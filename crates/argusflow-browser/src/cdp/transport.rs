//! 独立读写任务避免慢写阻塞读事件；一个 actor 拥有请求表。
use super::{
    connection::Request,
    lifecycle::{Health, PageState},
    protocol,
};
use crate::BrowserConfig;
use crate::BrowserError as Failure;
use argusflow_core::{FailureKind, Operation};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    net::TcpStream,
    sync::{mpsc, watch},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
struct Write {
    id: u64,
    message: Message,
    operation: Operation,
    effect: bool,
    document: Option<(Arc<PageState>, u64)>,
}
pub(crate) async fn connect(
    url: &str,
    config: &BrowserConfig,
    operation: &Operation,
) -> Result<Socket, Failure> {
    let socket_config = WebSocketConfig::default()
        .max_message_size(Some(config.max_message_bytes))
        .max_frame_size(Some(config.max_message_bytes));
    tokio::time::timeout(
        operation.remaining(),
        connect_async_with_config(url, Some(socket_config), false),
    )
    .await
    .map_err(|_| Failure::new(FailureKind::Timeout, "cdp_connect", "连接超时"))?
    .map(|(socket, _)| socket)
    .map_err(|error| {
        Failure::new(
            FailureKind::Unavailable,
            "cdp_connect",
            "无法连接 CDP WebSocket",
        )
        .with_source(error)
    })
}

pub(crate) async fn run(
    socket: Socket,
    mut receiver: mpsc::Receiver<Request>,
    health: Arc<Health>,
    mut stop: watch::Receiver<bool>,
    finished: watch::Sender<bool>,
    config: BrowserConfig,
) {
    let (mut writer, mut reader) = socket.split();
    let (write_sender, mut write_receiver) = mpsc::channel::<Write>(config.max_in_flight);
    let (event_sender, mut event_receiver) =
        mpsc::channel::<Result<Message, Failure>>(config.max_in_flight);
    let writer_events = event_sender.clone();
    let write_timeout = config.write_timeout;
    let writer_task = tokio::spawn(async move {
        while let Some(write) = write_receiver.recv().await {
            if write.document.as_ref().is_some_and(|(state, epoch)| {
                state.closed.load(std::sync::atomic::Ordering::Acquire)
                    || state.epoch.load(std::sync::atomic::Ordering::Acquire) != *epoch
            }) {
                write.operation.cancel();
                continue;
            }
            if write.operation.check("cdp_write").is_err() {
                continue;
            }
            if write.effect && write.operation.begin_effect("cdp_write").is_err() {
                continue;
            }
            let result = tokio::time::timeout(
                write.operation.remaining().min(write_timeout),
                writer.send(write.message),
            )
            .await;
            let error = match result {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(
                    Failure::new(
                        FailureKind::Unavailable,
                        "cdp_write",
                        format!("CDP 写入失败，id={}", write.id),
                    )
                    .with_source(error),
                ),
                Err(_) => Some(Failure::new(
                    FailureKind::Timeout,
                    "cdp_write",
                    "CDP 写入超时，连接将关闭",
                )),
            };
            if let Some(error) = error {
                let _ = writer_events.send(Err(error)).await;
                break;
            }
        }
    });
    let reader_task = tokio::spawn(async move {
        while let Some(message) = reader.next().await {
            let event = message.map_err(|error| {
                Failure::new(FailureKind::Unavailable, "cdp_read", "CDP 读取失败")
                    .with_source(error)
            });
            let failed = event.is_err();
            if event_sender.send(event).await.is_err() || failed {
                return;
            }
        }
        let _ = event_sender
            .send(Err(Failure::new(
                FailureKind::Unavailable,
                "cdp_read",
                "CDP 对端断开",
            )))
            .await;
    });
    let mut pending = HashMap::<u64, Request>::new();
    let mut next_id = 1u64;
    let mut tick = tokio::time::interval(Duration::from_millis(10));
    let terminal = loop {
        tokio::select! {
            _=stop.changed()=>{break Failure::new(FailureKind::Closed,"cdp_shutdown","CDP 连接关闭");}
            event=event_receiver.recv()=>{
                match event{
                    Some(Ok(Message::Text(text)))=>{if let Err(error)=protocol::receive(&text,&mut pending,&health){break error;}},
                    Some(Ok(Message::Ping(payload)))=>{
                        let operation=Operation::new(argusflow_core::OperationOptions::default());
                        if write_sender.try_send(Write{id:0,message:Message::Pong(payload),operation,effect:false,document:None}).is_err(){break Failure::new(FailureKind::Busy,"cdp_ping","CDP 写队列无法响应 ping");}
                    }
                    Some(Ok(Message::Pong(_)))=>{},
                    Some(Ok(Message::Close(_)))|None=>break Failure::new(FailureKind::Unavailable,"cdp_read","CDP 连接已关闭"),
                    Some(Ok(_))=>break Failure::new(FailureKind::Protocol,"cdp_read","CDP 收到意外帧类型"),
                    Some(Err(error))=>break error,
                }
                expire(&mut pending,&health);
            }
            request=receiver.recv()=>{
                let Some(request)=request else{break Failure::new(FailureKind::Closed,"cdp_queue","CDP 调用方已释放");};
                if let Err(error)=request.operation.check("cdp_queue").map_err(Failure::from).and_then(|_|health.check(request.session.as_deref())){let _=request.response.send(Err(error));continue;}
                let id=next_id;
                let Some(next)=next_id.checked_add(1)else{break Failure::new(FailureKind::ResourceLimit,"cdp_id","请求编号耗尽");};
                next_id=next;
                let mut payload=json!({"id":id,"method":request.method,"params":request.params});
                if let Some(session)=&request.session{payload["sessionId"]=json!(session);}
                let text=payload.to_string();
                if text.len()>config.max_message_bytes{let _=request.response.send(Err(Failure::new(FailureKind::ResourceLimit,"cdp_write","CDP 消息过大")));continue;}
                let write=Write{id,message:Message::Text(text.into()),operation:request.operation.clone(),effect:request.effect,document:request.document.clone()};
                if write_sender.try_send(write).is_err(){let _=request.response.send(Err(Failure::new(FailureKind::Busy,"cdp_write","CDP 写入队列已满")));continue;}
                pending.insert(id,request);
            }
            _=tick.tick()=>expire(&mut pending,&health),
        }
    };
    health.disconnect();
    protocol::fail_all(&mut pending, terminal.clone());
    receiver.close();
    while let Ok(request) = receiver.try_recv() {
        let _ = request
            .response
            .send(Err(request.operation.contextualize(terminal.clone())));
    }
    writer_task.abort();
    reader_task.abort();
    let _ = writer_task.await;
    let _ = reader_task.await;
    let _ = finished.send(true);
}

fn expire(pending: &mut HashMap<u64, Request>, health: &Health) {
    let failed = pending
        .iter()
        .filter_map(|(id, request)| {
            if request.response.is_closed() {
                request.operation.cancel();
            }
            if request.document.as_ref().is_some_and(|(state, epoch)| {
                state.epoch.load(std::sync::atomic::Ordering::Acquire) != *epoch
            }) {
                return Some((
                    *id,
                    Failure::new(
                        FailureKind::StaleHandle,
                        "cdp_document",
                        "请求所属文档已被替换",
                    ),
                ));
            }
            request
                .operation
                .check("cdp_pending")
                .map_err(Failure::from)
                .and_then(|_| health.check(request.session.as_deref()))
                .err()
                .map(|error| (*id, error))
        })
        .collect::<Vec<_>>();
    for (id, error) in failed {
        if let Some(request) = pending.remove(&id) {
            request.operation.cancel();
            let _ = request
                .response
                .send(Err(request.operation.contextualize(error)));
        }
    }
}
