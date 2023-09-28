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
pub struct Worker<C: Clone + std::fmt::Debug + Send + Sync, T: BackgroundTask<C>> {
    job_queue: JobQueue,
    db_pool: Connection,
    context: C,
    _task_marker: std::marker::PhantomData<T>,
}

impl<C, T> Worker<C, T>
where
    T: 'static + Serialize + for<'de> Deserialize<'de> + Send + Sync + BackgroundTask<C>,
    C: 'static + Clone + std::fmt::Debug + Send + Sync,
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
    ///
    /// This method starts the worker by continuously dequeuing jobs from the job queue and processing them.
    /// Each job is processed in a separate asynchronous task. If a job is found, it is processed and its status is updated in the database.
    /// If no job is found, the worker sleeps for a short duration before trying to dequeue the next job.
    /// Errors during job processing or database operations are logged.
    ///
    /// # Arguments
    /// * `self` - A reference to the worker instance.
    ///
    /// # Returns
    /// * `Result<(), WorkerError>` - This method returns a `Result` type. If the worker starts successfully, it returns `Ok(())`.
    ///   If an error occurs while dequeuing a job from the job queue, it returns `Err(WorkerError)`.
    ///
    /// # Errors
    /// * `WorkerError::JobQueue(JobQueueError)` - This error occurs when there is an issue dequeuing a job from the job queue.
    /// * `WorkerError::Database(DbErr)` - This error occurs when there is a database operation error.
    pub async fn start(&self) -> Result<(), WorkerError> {
        let db_pool = self.db_pool.clone();
        let context = self.context.clone();
        let job_queue = self.job_queue.clone();

        loop {
            // Dequeue the next job to process
            let job_option = job_queue.dequeue::<C, T>().await?;

            tokio::spawn({
                let db_pool = db_pool.clone();
                let context = context.clone();
                async move {
                    let db_conn = db_pool.get();
                    let db_pool_process = db_pool.clone();

                    if let Some(job) = job_option {
                        // Process the job
                        let job_tracking_result =
                            job_trackings::Entity::find_by_id(job.id).one(db_conn).await;

                        // Handle the error explicitly here
                        let model = match job_tracking_result {
                            Ok(model) => model,
                            Err(e) => {
                                error!("Error finding job tracking: {}", e);
                                return;
                            },
                        };

                        let Some(model) = model
                          else {
                            error!("Job tracking not found");
                            return;
                        };

                        let result = job.task.process(db_pool_process, context).await;

                        match result {
                            Ok(_) => {
                                let job_tracking_am =
                                    job_trackings::Entity::update_status(model, "completed");
                                if let Err(e) = job_tracking_am.update(db_conn).await {
                                    error!("Error updating job tracking: {}", e);
                                }
                                info!("Successfully processed job {}", job.id);
                            },
                            Err(e) => {
                                let job_tracking_am =
                                    job_trackings::Entity::update_status(model, "failed");
                                if let Err(e) = job_tracking_am.update(db_conn).await {
                                    error!("Error updating job tracking: {}", e);
                                }
                                error!("Error processing job {}: {}", job.id, e);
                            },
                        }
                    } else {
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    }
                }
            });
        }
    }
}
