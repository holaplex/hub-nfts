use hub_core::{
    thiserror, tokio,
    tracing::{error, info},
};
use sea_orm::{error::DbErr, ActiveModelTrait};
use serde::{Deserialize, Serialize};

use super::{
    job_queue::{JobQueue, JobQueueError},
    tasks::BackgroundTask,
};
use crate::{db::Connection, entities::job_trackings};

#[derive(thiserror::Error, Debug)]
pub enum WorkerError {
    #[error("Job queue error: {0}")]
    JobQueue(#[from] JobQueueError),
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
}
pub struct Worker<C: Clone, T: BackgroundTask<C>> {
    job_queue: JobQueue,
    db_pool: Connection,
    context: C,
    _task_marker: std::marker::PhantomData<T>,
}

impl<C, T> Worker<C, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + BackgroundTask<C>,
    C: Clone,
{
    pub fn new(job_queue: JobQueue, db_pool: Connection, context: C) -> Self {
        Self {
            job_queue,
            db_pool,
            context,
            _task_marker: std::marker::PhantomData,
        }
    }

    /// Start the worker
    /// # Arguments
    /// * `self` - The worker
    /// # Returns
    /// * `Result<(), WorkerError>` - The result of the operation
    /// # Errors
    /// * `WorkerError` - The error that occurred
    pub async fn start(&self) -> Result<(), WorkerError> {
        loop {
            // Dequeue the next job to process
            let job_option = self.job_queue.dequeue::<C, T>().await?;
            let db_conn = self.db_pool.get();

            if let Some(job) = job_option {
                // Process the job
                let model = job_trackings::Entity::find_by_id(job.id)
                    .one(db_conn)
                    .await?;

                if let Some(model) = model {
                    match job
                        .task
                        .process(self.db_pool.clone(), self.context.clone())
                        .await
                    {
                        Ok(_) => {
                            // If successful, update the status in the job_trackings table to "completed"
                            let job_tracking_am =
                                job_trackings::Entity::update_status(model, "completed");

                            job_tracking_am.update(db_conn).await?;

                            info!("Successfully processed job {}", job.id);
                        },
                        Err(e) => {
                            // If an error occurs, update the status in the job_trackings table to "failed"
                            let job_tracking_am =
                                job_trackings::Entity::update_status(model, "failed");

                            job_tracking_am.update(db_conn).await?;

                            // Log the error (or handle it in some other way)
                            error!("Error processing job {}: {}", job.id, e);
                        },
                    }
                } else {
                    error!("Job tracking record not found for job {}", job.id);
                }
            } else {
                // If no job was dequeued, you might want to add a delay here to avoid busy-waiting
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}
