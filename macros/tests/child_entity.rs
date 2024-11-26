use std::collections::HashSet;

use bagua::{entity::SysId, ChildEntity, Entity};

#[derive(PartialEq, Eq, Clone, Default, Copy, Hash, Debug)]
pub struct FileNodeId(i32);

impl SysId for FileNodeId {
    fn generate() -> Self {
        unreachable!()
    }
}

#[Entity]
pub struct FileNode {
    id: FileNodeId,
    #[entity(biz_id)]
    filename: String,
}

#[ChildEntity]
pub struct Dir {
    #[entity(parent)]
    node: FileNode,
    #[entity(foreign)]
    children: HashSet<FileNodeId>,
}
