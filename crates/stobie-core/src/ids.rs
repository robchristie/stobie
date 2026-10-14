use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident, $inner:ty) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            Hash,
            Serialize,
            Deserialize,
            PartialOrd,
            Ord,
        )]
        #[serde(transparent)]
        pub struct $name(pub $inner);
    };
}

id_type!(PaneId, u32);
id_type!(DockNodeId, u64);
