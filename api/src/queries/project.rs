use async_graphql::{Context, Object, Result};
use sea_orm::prelude::*;

use crate::objects::Project;

#[derive(Debug, Clone, Copy, Default)]
pub struct Query;

#[Object(name = "ProjectQuery")]
impl Query {
    /// Res
    ///
    /// # Errors
    /// This function fails if unable to set the project
    #[graphql(entity)]
    async fn find_project_by_id(
        &self,
        _ctx: &Context<'_>,
        #[graphql(key)] id: Uuid,
    ) -> Result<Project> {
        Ok(Project { id })
    }
}
