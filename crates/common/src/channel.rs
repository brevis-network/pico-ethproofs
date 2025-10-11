pub use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use anyhow::{Result, anyhow};
use derive_more::Constructor;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc::unbounded_channel};

/// unidirectional unbounded async channel, sender -> receiver
#[derive(Debug, Clone)]
pub struct SingleUnboundedChannel<T> {
    sender: Arc<UnboundedSender<T>>,
    receiver: Arc<Mutex<UnboundedReceiver<T>>>,
}

impl<T> Default for SingleUnboundedChannel<T> {
    fn default() -> Self {
        let (sender, receiver) = unbounded_channel();

        Self {
            sender: Arc::new(sender),
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }
}

impl<T> SingleUnboundedChannel<T> {
    pub fn sender(&self) -> Arc<UnboundedSender<T>> {
        self.sender.clone()
    }

    pub fn receiver(&self) -> Arc<Mutex<UnboundedReceiver<T>>> {
        self.receiver.clone()
    }

    pub fn send(&self, msg: T) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|err| anyhow!("failed to send msg: {err}"))
    }

    pub async fn recv(&self) -> Result<T> {
        let mut receiver = self.receiver.lock().await;
        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow!("channel closed"))
    }
}

/// duplex unbounded async endpoint includes a sender for type T and a receiver for type U
#[derive(Constructor, Debug, Clone)]
pub struct DuplexUnboundedEndpoint<T, U> {
    sender: Arc<UnboundedSender<T>>,
    receiver: Arc<Mutex<UnboundedReceiver<U>>>,
}

impl<T, U> DuplexUnboundedEndpoint<T, U> {
    pub fn sender(&self) -> Arc<UnboundedSender<T>> {
        self.sender.clone()
    }

    pub fn receiver(&self) -> Arc<Mutex<UnboundedReceiver<U>>> {
        self.receiver.clone()
    }

    pub fn send(&self, msg: T) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|err| anyhow!("failed to send msg: {err}"))
    }

    pub async fn recv(&self) -> Result<U> {
        let mut receiver = self.receiver.lock().await;
        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow!("channel closed"))
    }

    pub fn clone_sender(&self) -> Arc<UnboundedSender<T>> {
        Arc::new((*self.sender).clone())
    }
}

/// duplex unbounded async channel, endpoint1(sender<T>, receiver<U>) <-> endpoint2(sender<U>, Receiver<T>)
#[derive(Debug)]
pub struct DuplexUnboundedChannel<T, U> {
    endpoint1: Arc<DuplexUnboundedEndpoint<T, U>>,
    endpoint2: Arc<DuplexUnboundedEndpoint<U, T>>,
}

impl<T, U> Default for DuplexUnboundedChannel<T, U> {
    fn default() -> Self {
        let (sender1, receiver1) = unbounded_channel();
        let (sender2, receiver2) = unbounded_channel();

        let endpoint1 = Arc::new(DuplexUnboundedEndpoint::new(
            Arc::new(sender1),
            Arc::new(Mutex::new(receiver2)),
        ));
        let endpoint2 = Arc::new(DuplexUnboundedEndpoint::new(
            Arc::new(sender2),
            Arc::new(Mutex::new(receiver1)),
        ));

        Self {
            endpoint1,
            endpoint2,
        }
    }
}

impl<T, U> DuplexUnboundedChannel<T, U> {
    pub fn endpoint1(&self) -> Arc<DuplexUnboundedEndpoint<T, U>> {
        self.endpoint1.clone()
    }

    pub fn endpoint2(&self) -> Arc<DuplexUnboundedEndpoint<U, T>> {
        self.endpoint2.clone()
    }
}
