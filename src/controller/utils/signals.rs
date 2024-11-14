use std::sync::{ Arc};
use log::info;
use tokio::signal;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub fn notify_cancellation_token(token: &Arc<CancellationToken>){
    let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel::<()>();

    tokio::spawn({
        let token = token.clone();

        async move {
            //TODO: Unlikely it will run on windows node, but for that case its unsafe
            let mut sigterm = signal(SignalKind::terminate()).unwrap();

            tokio::select! {
                _ = signal::ctrl_c() => {
                   info!("Received ctrl+C, initiating graceful shutdown.");
                },
                _ = sigterm.recv() => {
                    info!("Received SIGTERM, initiating graceful shutdown.");
                },
                _ = shutdown_recv.recv() => {
                    info!("Received shutdown signal, initiating graceful shutdown.");
                }
            }


            token.cancel();
        }
    });

}