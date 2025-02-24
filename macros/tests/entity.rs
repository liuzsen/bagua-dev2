use std::{borrow::Borrow, collections::HashSet};

use bagua::{
    entity::{foreign::ForeignEntity, SysId},
    Entity, FieldGroup, ForeignEntity,
};

#[derive(
    PartialEq, Eq, Clone, Default, Copy, Hash, Debug, serde::Deserialize, serde::Serialize,
)]
pub struct FileNodeId(i32);

impl SysId for FileNodeId {
    fn generate() -> Self {
        unreachable!()
    }
}

#[Entity]
#[subset(FileNode1 {filename,})]
#[subset(FileNode2 {filename, meta})]
#[model_attr(derive(Debug))]
pub struct FileNode {
    id: FileNodeId,
    #[entity(biz_id)]
    filename: String,

    #[entity(biz_id)]
    filename2: Option<MyString>,

    #[entity(foreign)]
    foreign: HashSet<FileNodeForeign>,

    #[entity(group)]
    meta: FileMeta,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MyString(String);

#[derive(Debug, ForeignEntity, serde::Deserialize, serde::Serialize)]
pub struct FileNodeForeign {
    #[foreign(id)]
    id: FileNodeId,
    _other_field: String,
}

#[FieldGroup]
#[derive(Debug)]
pub struct FileMeta {
    size: u64,
    is_link: bool,
}
