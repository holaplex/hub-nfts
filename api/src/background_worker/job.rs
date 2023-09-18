use super::tasks::BackgroundTask;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Job<T: BackgroundTask> {
    pub id: i64,
    pub task: T,
}

impl<T: BackgroundTask> Serialize for Job<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let payload = self.task.payload();
        let payload_str = serde_json::to_string(&payload).map_err(serde::ser::Error::custom)?;

        let mut state = serializer.serialize_struct("Job", 3)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("task", &payload_str)?;
        state.end()
    }
}

impl<'de, T: BackgroundTask + for<'a> Deserialize<'a>> Deserialize<'de> for Job<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct JobHelper {
            id: i64,
            task: String,
        }

        let helper = JobHelper::deserialize(deserializer)?;

        let task: T = serde_json::from_str(&helper.task).map_err(serde::de::Error::custom)?;

        Ok(Job {
            id: helper.id,
            task,
        })
    }
}
