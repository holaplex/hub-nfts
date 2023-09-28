use std::{error::Error as StdError, fmt};

use hub_core::{prelude::*, thiserror};
use redis::{Client, RedisError};
use sea_orm::{error::DbErr, ActiveModelTrait};
use serde::{Deserialize, Serialize};
use serde_json::Error as SerdeJsonError;

use super::{job::Job, tasks::BackgroundTask};
use crate::{db::Connection, entities::job_trackings};

#[derive(Debug)]
struct LockError(String);

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl StdError for LockError {}

// Job queue errors
#[derive(thiserror::Error, Debug)]
pub enum JobQueueError {
    #[error("Redis error: {0}")]
    RedisConnection(#[from] RedisError),
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
    #[error("Serialization error: {0}")]
    Serde(#[from] SerdeJsonError),
    #[error("Background task error: {0}")]
    BackgroundTask(#[from] Error),
}
#[derive(Clone, Debug)]
pub struct JobQueue {
    client: Client,
    db_pool: Connection,
}

impl JobQueue {
    #[must_use]
    pub fn new(client: Client, db_pool: Connection) -> Self {
        Self { client, db_pool }
    }

    /// Enqueue a job
    /// # Arguments
    /// * `self` - The job queue
    /// * `task` - The task to enqueue
    /// # Returns
    /// * `Result<(), JobQueueError>` - The result of the operation
    /// # Errors
    /// * `JobQueueError` - The error that occurred
    pub async fn enqueue<C, T>(&self, task: T) -> Result<(), JobQueueError>
    where
        T: Serialize + for<'de> Deserialize<'de> + Send + Sync + BackgroundTask<C>,
        C: Clone + std::fmt::Debug + Send + Sync,
    {
        let mut conn = self.client.get_async_connection().await?;
        let db_conn = self.db_pool.get();

        let payload = task.payload()?;
        let new_job_tracking = job_trackings::Entity::create(task.name(), payload, "queued");

        let new_job_tracking = new_job_tracking.insert(db_conn).await?;

        let job_to_enqueue = Job::new(new_job_tracking.id, task);

        let payload = serde_json::to_string(&job_to_enqueue)?;

        redis::cmd("LPUSH")
            .arg("job_queue")
            .arg(payload)
            .query_async(&mut conn)
            .await?;

        Ok(())
    }

    /// Dequeue a job
    /// # Arguments
    /// * `self` - The job queue
    /// # Returns
    /// * `Result<Option<Job<C, T>>, JobQueueError>` - The result of the operation
    /// # Errors
    /// * `JobQueueError` - The error that occurred
    pub async fn dequeue<C, T>(&self) -> Result<Option<Job<C, T>>, JobQueueError>
    where
        T: Serialize + for<'de> Deserialize<'de> + Send + Sync + BackgroundTask<C>,
        C: Clone + std::fmt::Debug + Send + Sync,
    {
        let mut conn = self.client.get_async_connection().await?;
        let db_conn = self.db_pool.get();

        let res: Option<(String, String)> = redis::cmd("BRPOP")
            .arg("job_queue")
            .arg(0)
            .query_async(&mut conn)
            .await?;

        if let Some((_, job_details)) = res {
            let job: Job<C, T> = serde_json::from_str(&job_details)?;

            let job_tracking = job_trackings::Entity::find_by_id(job.id)
                .one(db_conn)
                .await?;

            if let Some(job_tracking) = job_tracking {
                if job_tracking.status == "completed" || job_tracking.status == "processing" {
                    return Ok(None);
                }

                let job_tracking_am =
                    job_trackings::Entity::update_status(job_tracking, "processing");

                job_tracking_am.save(db_conn).await?;
            }

            return Ok(Some(job));
        }

        Ok(None)
    }
}
