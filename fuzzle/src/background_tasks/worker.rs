use crate::{bot::BotError, database::Database, tags::TagManager};
use tracing::warn;
use std::{fmt::Debug, sync::Arc};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Debug)]
pub struct Worker<S>
where
    S: Debug + State,
{
    tx: mpsc::Sender<WorkerCommand<S>>,
}

type Responder<T> = oneshot::Sender<T>;

#[derive(Debug)]
pub enum WorkerCommand<S> {
    Update { new_state: Arc<S> },
    MaybeRecompute,
    GetState { resp: Responder<Arc<S>> },
}

impl<S> Worker<S>
where
    S: Debug + State + Send + 'static + Sync,
{
#[tracing::instrument(skip(database, tag_manager))]
    pub fn start(database: Database, tag_manager: Arc<TagManager>) -> Self {
        let (tx, mut rx) = mpsc::channel(100);
        let tx2 = tx.clone();
        tokio::spawn(async move {
            let Ok(mut state) = S::generate(database.clone(), Arc::clone(&tag_manager)).await else {
                return;
            };

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    WorkerCommand::Update { new_state } => {
                        state = new_state;
                    }
                    WorkerCommand::MaybeRecompute => {
                        let needs_recomputation = state.needs_recomputation(database.clone()).await;
                        let needs_recomputation = match needs_recomputation {
                            Err(err) => {
                                warn!("{err}");
                                continue;
                            }
                            Ok(val) => val,
                        };
                        if needs_recomputation {
                            let database = database.clone();
                            let tag_manager = Arc::clone(&tag_manager);
                            let tx = tx2.clone();
                            tokio::spawn(async move {
                                let new_state = S::generate(database, tag_manager).await;
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
                        if let Err(_) = resp.send(state.clone()) {
                            warn!("could not send state");
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    /// does not wait for completion
#[tracing::instrument(skip(self))]
    pub async fn maybe_recompute(&self) -> Result<(), BotError> {
        self.tx
            .send(WorkerCommand::MaybeRecompute)
            .await
            .map_err(|err| anyhow::anyhow!("send error"))?;
        Ok(())
    }

#[tracing::instrument(skip(self, command), err(Debug))]
    pub async fn execute<A, B>(&self, command: A) -> Result<B, BotError>
    where
        A: Comm<S> + Comm<S, ReturnType = B>,
    {
        let (resp, receive) = oneshot::channel();
        self.tx
            .send(WorkerCommand::GetState { resp })
            .await
            .map_err(|err| anyhow::anyhow!("send error"))?;
        let res = receive.await?;
        command.apply(res).await
    }
}

pub trait State {
    fn generate(
        database: Database,
        tag_manager: Arc<TagManager>,
    ) -> impl std::future::Future<Output = Result<Arc<Self>, BotError>> + Send;
    fn needs_recomputation(
        &self,
        database: Database,
    ) -> impl std::future::Future<Output = Result<bool, BotError>> + Send;
}

pub trait Comm<S> {
    type ReturnType;

    fn apply(
        &self,
        state: Arc<S>,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, BotError>> + Send;
}
