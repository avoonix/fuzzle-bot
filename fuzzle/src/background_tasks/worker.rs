use crate::{bot::{InternalError}, database::Database};
use tracing::warn;
use std::{fmt::Debug, marker::PhantomData, sync::Arc};
use tokio::sync::{mpsc, oneshot};

use super::TagManagerWorker;

#[derive(Clone, Debug)]
pub struct Worker<S, D> // state, dependencies
where
    S: Debug + State<D>,
    D: Clone,
{
    tx: mpsc::Sender<WorkerCommand<S, D>>,
    phantom: PhantomData<D>,
}

type Responder<T> = oneshot::Sender<T>;

#[derive(Debug)]
pub enum WorkerCommand<S, D> {
    Update { new_state: Arc<S> },
    MaybeRecompute,
    GetState { resp: Responder<(Arc<S>, D)> }, // TODO: arc for D?
}

impl<S, D> Worker<S, D>
where
    S: Debug + State<D> + Send + 'static + Sync,
    D: Clone + Send + 'static + Sync,
{
#[tracing::instrument(skip(deps))]
    pub fn start(deps: D) -> Self {
        let (tx, mut rx) = mpsc::channel(100);
        let tx2 = tx.clone();
        tokio::spawn(async move {
            let Ok(mut state) = S::generate(deps.clone()).await else {
                return;
            };

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    WorkerCommand::Update { new_state } => {
                        state = new_state;
                    }
                    WorkerCommand::MaybeRecompute => {
                        let needs_recomputation = state.needs_recomputation(deps.clone()).await;
                        let needs_recomputation = match needs_recomputation {
                            Err(err) => {
                                warn!("{err}");
                                continue;
                            }
                            Ok(val) => val,
                        };
                        if needs_recomputation {
                            let deps = deps.clone();
                            let tx = tx2.clone();
                            tokio::spawn(async move {
                                let new_state = S::generate(deps.clone()).await;
                                let new_state = match new_state {
                                    Err(err) => {
                                        warn!("{err}");
                                        return;
                                    }
                                    Ok(val) => val,
                                };
                                if let Err(err) = tx.send(WorkerCommand::Update { new_state }).await
                                {
                                    warn!("{err}");
                                }
                            });
                        }
                    }
                    WorkerCommand::GetState { resp } => {
                        if let Err(_) = resp.send((state.clone(), deps.clone())) {
                            warn!("could not send state");
                        }
                    }
                }
            }
        });

        Self { tx, phantom: PhantomData }
    }

    /// does not wait for completion
#[tracing::instrument(skip(self))]
    pub async fn maybe_recompute(&self) -> Result<(), InternalError> {
        self.tx
            .send(WorkerCommand::MaybeRecompute)
            .await
            .map_err(|err| anyhow::anyhow!("send error"))?;
        Ok(())
    }

#[tracing::instrument(skip(self, command), err(Debug))]
    pub async fn execute<A, B>(&self, command: A) -> Result<B, InternalError>
    where
        A: Comm<S, D> + Comm<S, D, ReturnType = B>,
    {
        let (resp, receive) = oneshot::channel();
        self.tx
            .send(WorkerCommand::GetState { resp })
            .await
            .map_err(|err| anyhow::anyhow!("send error"))?;
        let res = receive.await?;
        command.apply(res.0, res.1).await
    }
}

pub trait State<D> {
    fn generate(
        deps: D,
    ) -> impl std::future::Future<Output = Result<Arc<Self>, InternalError>> + Send;
    fn needs_recomputation(
        &self,
        deps: D,
    ) -> impl std::future::Future<Output = Result<bool, InternalError>> + Send;
}

pub trait Comm<S, D> {
    type ReturnType;

    fn apply(
        &self,
        state: Arc<S>,
        deps: D,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, InternalError>> + Send;
}
