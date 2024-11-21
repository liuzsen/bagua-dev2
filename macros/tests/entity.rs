use std::collections::HashSet;

use bagua::{entity::SysId, Entity, GuardedStruct};

#[derive(PartialEq, Eq, Clone, Default, Copy, Hash, Debug)]
pub struct FileNodeId(i32);

impl SysId for FileNodeId {
    fn generate() -> Self {
        todo!()
    }
}

#[Entity]
#[subset(FileNode1 {filename,})]
#[model_attr(derive(Debug))]
pub struct FileNode {
    id: FileNodeId,
    #[entity(biz_id)]
    filename: String,
    #[entity(flatten)]
    permits: Permits,
    #[entity(foreign)]
    foreign: HashSet<FileNodeId>,
}

#[GuardedStruct]
#[derive(PartialEq, Eq, Clone, Copy)]
#[model_attr(derive(Debug))]
pub struct Permits {
    read: bool,
    write: bool,
    execute: bool,
}
