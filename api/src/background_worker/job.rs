use serde::{Deserialize, Serialize};

use super::tasks::BackgroundTask;

#[derive(Serialize, Deserialize, Debug)]
pub struct Job<C: Clone + Send + Sync, T: Serialize + Send + Sync + BackgroundTask<C>> {
    pub id: i32,
    pub task: T,
    _context_marker: std::marker::PhantomData<C>,
}

impl<C: Clone + Send + Sync, T> Job<C, T>
where
    T: Serialize + Send + Sync + BackgroundTask<C>,
{
    #[must_use]
    pub fn new(id: i32, task: T) -> Self {
        Self {
            id,
            task,
            _context_marker: std::marker::PhantomData,
        }
    }
}
