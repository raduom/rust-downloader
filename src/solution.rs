use std::time::Duration;

use crate::statement::*;
use async_trait::async_trait;
use tokio::{
    sync::mpsc::{self, Sender},
    time::sleep,
};

#[async_trait]
pub trait Solution {
    async fn solve(repositories: Vec<ServerName>) -> Option<Binary>;
}

pub struct Solution0;

pub enum FailedDownload {
    Retry(u8, ServerName, ServerError),
    GivingUp(ServerName),
}

pub async fn start_download(
    channel: Sender<Result<Binary, FailedDownload>>,
    server_name: ServerName,
    retries: u8,
) {
    let mut timeout_scaling = 1;

    for retry in 1..=retries {
        match download(server_name.clone()).await {
            Ok(binary) => match channel.send(Ok(binary)).await {
                Ok(_) => return,
                Err(err) => {
                    eprintln!("Caught error while sending found binary message: {}", err);
                }
            },
            Err(server_error) => {
                if let Err(err) = channel
                    .send(Err(FailedDownload::Retry(
                        retry,
                        server_name.clone(),
                        server_error,
                    )))
                    .await
                {
                    eprintln!(
                        "Caught error while sending failed download message: {}",
                        err
                    );
                }

                timeout_scaling *= 5;
                sleep(Duration::from_millis(timeout_scaling * 100)).await;
            }
        }
    }

    if let Err(err) = channel
        .send(Err(FailedDownload::GivingUp(server_name.clone())))
        .await
    {
        eprintln!("Caught error while sending giving up message: {}", err);
    }
}

#[async_trait]
impl Solution for Solution0 {
    async fn solve(repositories: Vec<ServerName>) -> Option<Binary> {
        let (send, mut receive) = mpsc::channel(32);

        let handles: Vec<_> = repositories
            .iter()
            .map(|server_name| tokio::spawn(start_download(send.clone(), server_name.clone(), 5)))
            .collect();

        // Closing this task's channel so we can get a `None` returned when all
        // the other tasks finish.
        drop(send);

        loop {
            match receive.recv().await {
                Some(Ok(binary)) => {
                    // Cleanup
                    handles.iter().for_each(|h| h.abort());
                    return Some(binary);
                }
                // All channels are out of scope/dropped, which means that the
                // tasks have finished, so there is no cleanup necessary.
                None => return None,
                _ => continue,
            }
        }
    }
}
