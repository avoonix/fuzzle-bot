use log::{info, warn};
use teloxide::types::Update;

#[derive(Clone, Debug)]
pub struct Timer {
    start: chrono::DateTime<chrono::Utc>,
    update: Update
}

impl Timer {
    pub fn new(update: Update) -> Self {
        Self {
            start: chrono::Utc::now(),
            update,
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let elapsed = chrono::Utc::now() - self.start;
        if elapsed > chrono::Duration::seconds(2) {
            warn!("slow response ({}ms): {:?}", elapsed.num_milliseconds(), self.update);
        } else {
            info!("responded within {}ms", elapsed.num_milliseconds());
        }
    }
}
