use super::job_queue::JobQueue;
use crate::db::Connection;
use hub_core::{
    tokio,
    tracing::{error, info},
};
use std::sync::Arc;

pub struct Worker {
    job_queue: Arc<JobQueue>,
    db_pool: Arc<Connection>,
}

impl Worker {
    pub fn new(job_queue: Arc<JobQueue>, db_pool: Connection) -> Self {
        Self {
            job_queue,
            db_pool: Arc::new(db_pool),
        }
    }

    pub async fn start(&self) {
        loop {
            if let Ok(Some(mut job)) = self.job_queue.dequeue().await {
                let job_queue_clone = self.job_queue.clone();
                let job_id = job.id;
                let db_pool_clone = Arc::clone(&self.db_pool);
                tokio::spawn(async move {
                    if JobTracking::find_by_id(job_id, &db_pool_clone)
                        .await?
                        .is_none()
                    {

                        // Create a new record in the database
                        JobTracking::create(
                            job_id,
                            "JobType",
                            job.task.payload(),
                            "processing",
                            &db_pool_clone,
                        )
                        .await?;

                        // sora elle espinola
                        // Process the job using the trait method
                        match job.task.process() {
                            Ok(_) => {
                                // Update the job status in the database to "completed"
                                JobTracking::update_status(&job, "completed", &db_pool_clone)
                                    .await
                                    .unwrap();
                            },
                            Err(e) => {
                                println!("Job processing failed: {}", e);

                                // Re-queue the job and update the job status in the database to "queued"
                                job_queue_clone
                                    .enqueue(&job)
                                    .await
                                    .expect("Failed to re-queue job");
                                JobTracking::update_status(&job, "queued", &db_pool_clone)
                                    .await
                                    .unwrap();
                            },
                        }
                    } else {
                        info!("Duplicate job detected, skipping: {}", job_id);
                    }
                    Ok::<(), sea_orm::DbErr>(())
                })
                .await
                .unwrap_or_else(|e| {
                    error!("An error occurred: {}", e);

                    Ok::<(), sea_orm::DbErr>(())
                });
            }
        }
    }
}
