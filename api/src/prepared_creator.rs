use crate::proto::{Creator, SolanaCreator};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreparedCreator {
    pub address: String,
    pub share: i32,
    pub verified: bool,
}

impl From<Creator> for PreparedCreator {
    #[inline]
    fn from(value: Creator) -> Self {
        let Creator {
            address,
            share,
            verified,
        } = value;
        PreparedCreator {
            address,
            share,
            verified,
        }
    }
}

impl From<PreparedCreator> for Creator {
    #[inline]
    fn from(value: PreparedCreator) -> Self {
        let PreparedCreator {
            address,
            share,
            verified,
        } = value;
        Self {
            address,
            share,
            verified,
        }
    }
}
