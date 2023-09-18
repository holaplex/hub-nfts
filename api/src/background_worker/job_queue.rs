use super::{job::Job, tasks::BackgroundTask};
use crate::db::Connection;
use redis::AsyncCommands;
use redis::Client;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct LockError(String);

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for LockError {}

pub struct JobQueue {
    client: Arc<Mutex<Client>>,
    db_pool: Arc<Connection>,
}

impl JobQueue {
    pub async fn new(redis_url: &str, db_pool: Connection) -> Self {
        let client = Client::open(redis_url).expect("Failed to create Redis client");
        Self {
            client: Arc::new(Mutex::new(client)),
            db_pool: Arc::new(db_pool),
        }
    }

    pub async fn enqueue<T: BackgroundTask>(
        &self,
        job: &Job<T>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client_guard = self
            .client
            .lock()
            .map_err(|e| Box::new(LockError(e.to_string())) as Box<dyn Error>)?;
        let mut conn = client_guard.get_async_connection().await?;

        let payload = serde_json::to_string(&job.task.payload())?;

        redis::cmd("LPUSH")
            .arg("job_queue")
            .arg(payload)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn dequeue<T: BackgroundTask>(
        &self,
    ) -> Result<Option<Job<T>>, Box<dyn std::error::Error>> {
        let client_guard = self
            .client
            .lock()
            .map_err(|e| Box::new(LockError(e.to_string())) as Box<dyn Error>)?;
        let mut conn = client_guard.get_async_connection()?;

        let payload: Option<String> = redis::cmd("RPOP")
            .arg("job_queue")
            .query_async(&mut conn)
            .await?;

        if let Some(payload) = payload {
            let task: Box<dyn BackgroundTask> = serde_json::from_str(&payload)?;
            let job = Job {
                id: generate_unique_id(), // You would need to implement this function
                task,
            };
            Ok(Some(job))
        } else {
            Ok(None)
        }
    }
}
