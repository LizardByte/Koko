//! Streaming routes for remote desktop.

use crate::auth::UserGuard;
use crate::streaming::StreamSession;
use rocket::{State, get};
use rocket_ws as ws;
use std::sync::Arc;

/// WebSocket endpoint for streaming
#[get("/stream")]
pub async fn stream(
    ws: ws::WebSocket,
    _auth: UserGuard,
    session: &State<Arc<StreamSession>>,
) -> ws::Channel<'static> {
    let session = session.inner().clone();

    ws.channel(move |stream| {
        Box::pin(async move {
            if let Err(e) = session.handle_connection(stream).await {
                log::error!("Streaming error: {}", e);
            }
            Ok(())
        })
    })
}
