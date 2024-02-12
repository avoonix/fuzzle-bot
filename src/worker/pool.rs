// worker is used for handling non-critical tasks such as periodically fetching stickers from the database
// and updating the sticker cache, as well as periodically updating the tag cache

use chrono::Duration;
use chrono::NaiveDateTime;
use std::sync::Arc;

use teloxide::types::UserId;
use tokio::sync::mpsc;
use tokio::sync::Semaphore;
use tokio::time::sleep;

use crate::bot::log_error_and_send_to_admin;
use crate::bot::Bot;

use crate::database::Database;
use crate::sticker::analyze_n_stickers;
use crate::sticker::calculate_visual_hash;
use crate::Paths;

use super::command::AdminMessage;
use super::command::Command;

#[derive(Clone, Debug)]
pub struct WorkerPool {
    tx: mpsc::Sender<Command>,
}

impl WorkerPool {
    pub async fn start_manager(
        bot: Bot,
        admin_id: UserId,
        database: Database,
        queue_length: usize,
        concurrency: usize,
        paths: Paths,
    ) -> (tokio::task::JoinHandle<()>, Self) {
        let (tx, mut rx) = mpsc::channel(queue_length);
        let worker = Self { tx };
        let worker_clone = worker.clone();
        let bot_clone = bot.clone();
        let database_clone = database.clone();
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let manager = tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                let permit = semaphore.clone().acquire_owned().await.unwrap(); // TODO: close semaphore when stopping the server
                let bot = bot.clone();
                let worker = worker.clone();
                let database = database.clone();
                let paths = paths.clone();
                tokio::spawn(async move {
                    let result = cmd
                        .execute(bot.clone(), admin_id, worker, database, paths)
                        .await;
                    match result {
                        Ok(()) => {}
                        Err(err) => log_error_and_send_to_admin(err, bot, admin_id).await,
                    }
                    drop(permit);
                });
            }
        });

        // scheduled tasks
        // TODO: make intervals and counts configurable

        let worker = worker_clone.clone();
        tokio::spawn(async move {
            loop {
                // fetching 200 sets every 4 hours is 8400 sets per week
                sleep(Duration::hours(4).to_std().unwrap()).await;
                worker
                    .tx
                    .send(Command::RefetchScheduled { count: 200 })
                    .await
                    .unwrap();
            }
        });

        let worker = worker_clone.clone();
        tokio::spawn(async move {
            loop {
                worker.tx.send(Command::SendReport).await.unwrap();
                sleep(Duration::hours(24).to_std().unwrap()).await;
            }
        });

        let worker = worker_clone.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::days(7).to_std().unwrap()).await;
                worker.tx.send(Command::SendExport).await.unwrap();
            }
        });

        (manager, worker_clone)
    }

    pub async fn dispatch_message_to_admin(&self, source_user: UserId, msg: AdminMessage) {
        // does not wait for a response
        let cmd = Command::SendMessageToAdmin { source_user, msg };
        self.tx.send(cmd).await.unwrap();
    }

    pub async fn process_sticker_set(&self, source_user: Option<UserId>, set_name: String) {
        let cmd = Command::ProcessStickerSet {
            set_name,
            source_user,
        };
        self.tx.send(cmd).await.unwrap();
    }

    pub async fn process_set_of_sticker(
        &self,
        source_user: Option<UserId>,
        sticker_unique_id: String,
    ) {
        let cmd = Command::ProcessSetOfSticker {
            sticker_unique_id,
            source_user,
        };
        self.tx.send(cmd).await.unwrap();
    }

    pub async fn refetch_all_sets(&self) {
        self.tx.send(Command::RefetchAllSets).await.unwrap();
    }

    pub async fn send_report(&self) {
        self.tx.send(Command::SendReport).await.unwrap();
    }
}
