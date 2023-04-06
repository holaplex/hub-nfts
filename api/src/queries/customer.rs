use async_graphql::{Context, Object, Result};
use sea_orm::prelude::*;

use crate::objects::Customer;

#[derive(Debug, Clone, Copy, Default)]
pub struct Query;

#[Object(name = "CustomerQuery")]
impl Query {
    /// Res
    ///
    /// # Errors
    /// This function fails if unable to set the project
    #[graphql(entity)]
    async fn find_customer_by_id(
        &self,
        _ctx: &Context<'_>,
        #[graphql(key)] id: Uuid,
        addresses: Option<Vec<String>>,
    ) -> Result<Customer> {
        Ok(Customer { id, addresses })
    }
}
