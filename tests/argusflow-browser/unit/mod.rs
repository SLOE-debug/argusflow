//! 本地协议替身，不启动浏览器。
mod managed;
#[path = "../support/mock.rs"]
mod mock;
mod ownership;
mod page;
mod transport;

use crate::BrowserConfig;
use argusflow_core::Operation;
use futures_util::{SinkExt, StreamExt};
use mock::{Mock, options};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};
