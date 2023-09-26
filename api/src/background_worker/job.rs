use serde::{
    de::{Deserialize, Deserializer, Error as DeError},
    Serialize,
};

use super::tasks::BackgroundTask;

#[derive(Serialize, Debug)]
pub struct Job<C: Clone, T: Serialize + Send + Sync + BackgroundTask<C>> {
    pub id: i64,
    pub task: T,
    _context_marker: std::marker::PhantomData<C>,
}

impl<C: Clone, T> Job<C, T>
where
    T: Serialize + Send + Sync + BackgroundTask<C>,
{
    #[must_use]
    pub fn new(id: i64, task: T) -> Self {
        Self {
            id,
            task,
            _context_marker: std::marker::PhantomData,
        }
    }
}

impl<'de, C, T> Deserialize<'de> for Job<C, T>
where
    C: Clone,
    T: Serialize + Send + Sync + BackgroundTask<C>,
    T: for<'a> Deserialize<'a>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (id, task) = Deserialize::deserialize(deserializer).map_err(DeError::custom)?;

        Ok(Job::new(id, task))
    }
}
