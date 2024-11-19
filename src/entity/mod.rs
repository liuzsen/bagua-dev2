use updater::Updater;

pub mod field;
pub mod flatten;
pub mod foreign;
pub mod model;
pub mod parent;
pub mod subset;
pub mod updater;

pub trait Entity: GuardedStruct {
    type Id<'a>: Eq;

    type SysId: Eq;
}

pub trait SysId {
    fn generate() -> Self;
}

pub trait ChildEntity: Entity {
    type Parent: Entity;
}

pub trait GuardedStruct {
    type Updater: Updater<GuardedStruct = Self>;

    fn update_fields(&mut self, updater: Self::Updater);
}

#[allow(unused)]
#[cfg(test)]
mod tests {
    use indexmap::IndexSet;
    use primitives::*;
    mod primitives {
        use std::borrow::Cow;

        pub struct MediaDuration(std::time::Duration);

        pub struct MultiLangText {
            pub origin: Cow<'static, str>,
            pub en_us: Option<Cow<'static, str>>,
            pub zh_hans_cn: Option<Cow<'static, str>>,
            pub zh_hant_tw: Option<Cow<'static, str>>,
        }

        #[derive(PartialEq, Eq, Clone, Hash, Debug)]
        pub struct OuterId {
            pub id: String,
            pub from: OuterIdType,
        }

        #[derive(PartialEq, Eq, Clone, Hash, Debug)]
        pub enum OuterIdType {
            Imdb,
            Tmdb,
            Douban,
            Filename,
        }
    }

    use foreign::*;
    mod foreign {
        use serde::{Deserialize, Serialize};

        use crate::entity::SysId;

        pub enum Resolution {
            Unknown,
        }

        #[derive(PartialEq, Eq, Clone, Hash, Debug)]
        pub struct VideoId(i64);

        #[derive(PartialEq, Eq, Clone, Hash, Debug)]
        pub struct GenreId(i64);

        #[derive(PartialEq, Eq, Clone, Hash, Debug)]
        pub struct MediaId(i64);

        impl MediaId {
            pub fn generate() -> Self {
                Self(0)
            }
        }

        impl SysId for MediaId {
            fn generate() -> Self {
                Self::generate()
            }
        }

        #[derive(Serialize, Deserialize)]
        pub struct MediaAlbumId(i64);

        #[derive(Serialize, Deserialize)]
        pub struct PictureId(i64);
    }

    pub struct MediaModel {
        pub outer_id: Option<OuterId>,
        pub title: MultiLangText,
        pub tagline: Option<MultiLangText>,
        pub overview: Option<MultiLangText>,
        pub rating: Option<f64>,
        pub genres: IndexSet<GenreId>,
        pub album: MediaAlbumModel,
    }
    const _: () = {
        use crate::entity::model::Model;
        impl Model for MediaModel {
            type Entity = Media;
            fn build_entity(self) -> Self::Entity {
                Media {
                    id: <MediaId>::generate(),
                    outer_id: crate::entity::field::Reset::reset(self.outer_id),
                    title: crate::entity::field::Reset::reset(self.title),
                    tagline: crate::entity::field::Reset::reset(self.tagline),
                    overview: crate::entity::field::Reset::reset(self.overview),
                    rating: crate::entity::field::Reset::reset(self.rating),
                    genres: crate::entity::field::Reset::reset(self.genres),
                    album: crate::entity::field::Reset::reset(self.album.build()),
                }
            }
        }
    };

    impl MediaModel {
        #[allow(unused)]
        pub fn build_entity(self) -> Media {
            crate::entity::model::Model::build_entity(self)
        }
    }

    #[derive(Default)]
    pub struct MediaUpdater {
        pub outer_id: Option<Option<OuterId>>,
        pub title: Option<MultiLangText>,
        pub tagline: Option<Option<MultiLangText>>,
        pub overview: Option<Option<MultiLangText>>,
        pub rating: Option<Option<f64>>,
        pub genres: Option<IndexSet<GenreId>>,
        pub add_genres: Option<IndexSet<GenreId>>,
        pub remove_genres: Option<IndexSet<GenreId>>,
        pub album: MediaAlbumUpdater,
    }

    impl Updater for MediaUpdater {
        type GuardedStruct = Media;
    }

    #[repr(C)]
    pub struct Media {
        id: MediaId,
        outer_id: crate::entity::field::Field<Option<OuterId>>,
        title: crate::entity::field::Field<MultiLangText>,
        tagline: crate::entity::field::Field<Option<MultiLangText>>,
        overview: crate::entity::field::Field<Option<MultiLangText>>,
        rating: crate::entity::field::Field<Option<f64>>,
        genres: crate::entity::foreign::ForeignEntities<IndexSet<GenreId>>,
        album: crate::entity::flatten::FlattenStruct<MediaAlbum>,
    }

    impl GuardedStruct for Media {
        type Updater = MediaUpdater;

        fn update_fields(&mut self, updater: Self::Updater) {
            self.outer_id.update_value(updater.outer_id);
            self.title.update_value(updater.title);
            self.tagline.update_value(updater.tagline);
            self.overview.update_value(updater.overview);
            self.rating.update_value(updater.rating);

            self.genres.update_value(updater.genres);
            if let Some(values) = updater.add_genres {
                for t in values {
                    self.genres.add(t);
                }
            }
            if let Some(values) = updater.remove_genres {
                for t in values {
                    self.genres.remove(t);
                }
            }

            self.album.update_fields(updater.album);
        }
    }

    #[derive(PartialEq, Eq, Clone, Hash, Debug)]
    pub enum MediaIdent {
        SysId(MediaId),
        OuterId(Option<OuterId>),
    }

    impl From<MediaId> for MediaIdent {
        fn from(value: MediaId) -> Self {
            Self::SysId(value)
        }
    }
    impl From<Option<OuterId>> for MediaIdent {
        fn from(value: Option<OuterId>) -> Self {
            Self::OuterId(value)
        }
    }
    const _: () = {
        use crate::entity::Entity;
        impl Entity for Media {
            type Id<'a> = MediaIdent;

            type SysId = MediaId;
        }
    };
    impl core::ops::Deref for Media {
        type Target = MediaReadOnly;
        fn deref(&self) -> &Self::Target {
            unsafe { &*(self as *const Self as *const Self::Target) }
        }
    }
    #[repr(C)]
    pub struct MediaReadOnly {
        pub id: MediaId,
        pub outer_id: crate::entity::field::Field<Option<OuterId>>,
        pub title: crate::entity::field::Field<MultiLangText>,
        pub tagline: crate::entity::field::Field<Option<MultiLangText>>,
        pub overview: crate::entity::field::Field<Option<MultiLangText>>,
        pub rating: crate::entity::field::Field<Option<f64>>,
        pub genres: crate::entity::foreign::ForeignEntities<IndexSet<GenreId>>,
        pub album: crate::entity::flatten::FlattenStruct<MediaAlbum>,
    }
    impl Media {
        pub fn read_only(&self) -> &MediaReadOnly {
            ::core::ops::Deref::deref(self)
        }
    }
    #[allow(unused)]
    pub use media_subsets::*;
    pub mod media_subsets {
        #![allow(unused_imports)]
        use super::*;
        pub struct Media2 {
            pub id: MediaId,
            pub album: MediaAlbum,
        }

        impl crate::entity::subset::Subset for Media2 {
            type Entity = Media;
            fn to_entity(self) -> Self::Entity {
                Media {
                    id: crate::entity::field::Unchanged::unchanged(self.id),
                    album: crate::entity::field::Unchanged::unchanged(self.album),
                    outer_id: crate::entity::field::Unloaded::unloaded(),
                    title: crate::entity::field::Unloaded::unloaded(),
                    tagline: crate::entity::field::Unloaded::unloaded(),
                    overview: crate::entity::field::Unloaded::unloaded(),
                    rating: crate::entity::field::Unloaded::unloaded(),
                    genres: crate::entity::field::Unloaded::unloaded(),
                }
            }
        }
        impl From<Media2> for Media {
            fn from(subset: Media2) -> Self {
                crate::entity::subset::Subset::to_entity(subset)
            }
        }
        pub struct MediaMini {
            pub id: MediaId,
        }
        impl crate::entity::subset::Subset for MediaMini {
            type Entity = Media;
            fn to_entity(self) -> Self::Entity {
                Media {
                    id: self.id,
                    outer_id: crate::entity::field::Unloaded::unloaded(),
                    title: crate::entity::field::Unloaded::unloaded(),
                    tagline: crate::entity::field::Unloaded::unloaded(),
                    overview: crate::entity::field::Unloaded::unloaded(),
                    rating: crate::entity::field::Unloaded::unloaded(),
                    genres: crate::entity::field::Unloaded::unloaded(),
                    album: crate::entity::field::Unloaded::unloaded(),
                }
            }
        }
        impl From<MediaMini> for Media {
            fn from(subset: MediaMini) -> Self {
                crate::entity::subset::Subset::to_entity(subset)
            }
        }
    }

    pub struct MediaVideo {
        pub video_id: VideoId,
        pub resolution: Resolution,
    }

    use serde::{Deserialize, Serialize};
    use tv_episode::*;
    mod tv_episode {
        use crate::entity::Entity;

        use super::*;

        #[allow(unused)]
        pub struct TvEpisodeModel {
            pub media: MediaModel,
            pub videos: Vec<MediaVideo>,
            pub belongs_to_season: MediaId,
            pub belongs_to_tv: MediaId,
            pub name: MultiLangText,
            pub episode_number: u16,
            pub duration: Option<MediaDuration>,
        }
        const _: () = {
            use crate::entity::model::Model;
            impl Model for TvEpisodeModel {
                type Entity = TvEpisode;
                fn build_entity(self) -> Self::Entity {
                    TvEpisode {
                        media: crate::entity::field::Reset::reset(self.media.build_entity()),
                        videos: crate::entity::field::Reset::reset(self.videos),
                        belongs_to_season: crate::entity::field::Reset::reset(
                            self.belongs_to_season,
                        ),
                        belongs_to_tv: crate::entity::field::Reset::reset(self.belongs_to_tv),
                        name: crate::entity::field::Reset::reset(self.name),
                        episode_number: crate::entity::field::Reset::reset(self.episode_number),
                        duration: crate::entity::field::Reset::reset(self.duration),
                    }
                }
            }
        };
        impl TvEpisodeModel {
            #[allow(unused)]
            pub fn build_entity(self) -> TvEpisode {
                crate::entity::model::Model::build_entity(self)
            }
        }

        type EpisodeNumber = u16;

        #[derive(Default)]
        pub struct TvEpisodeUpdater {
            pub media: <Media as GuardedStruct>::Updater,
            pub videos: Option<Vec<MediaVideo>>,
            pub belongs_to_season: Option<MediaId>,
            pub belongs_to_tv: Option<MediaId>,
            pub name: Option<MultiLangText>,
            pub episode_number: Option<EpisodeNumber>,
            pub duration: Option<Option<MediaDuration>>,
        }

        impl Updater for TvEpisodeUpdater {
            type GuardedStruct = TvEpisode;
        }

        impl TvEpisode {
            pub fn update(&mut self, updater: TvEpisodeUpdater) {
                self.media.update_fields(updater.media);
                self.videos.update_value(updater.videos);
                self.belongs_to_season
                    .update_value(updater.belongs_to_season);
                self.belongs_to_tv.update_value(updater.belongs_to_tv);
                self.name.update_value(updater.name);
                self.episode_number.update_value(updater.episode_number);
                self.duration.update_value(updater.duration);
            }
        }

        #[repr(C)]
        pub struct TvEpisode {
            media: crate::entity::parent::ParentEntity<Media>,
            videos: crate::entity::field::Field<Vec<MediaVideo>>,
            belongs_to_season: crate::entity::field::Field<MediaId>,
            belongs_to_tv: crate::entity::field::Field<MediaId>,
            name: crate::entity::field::Field<MultiLangText>,
            episode_number: crate::entity::field::Field<EpisodeNumber>,
            duration: crate::entity::field::Field<Option<MediaDuration>>,
        }

        impl GuardedStruct for TvEpisode {
            type Updater = TvEpisodeUpdater;

            fn update_fields(&mut self, updater: Self::Updater) {
                todo!()
            }
        }

        const _: () = {
            use crate::entity::Entity;
            impl Entity for TvEpisode {
                type Id<'a> = <Media as Entity>::Id<'a>;

                type SysId = <Media as Entity>::SysId;
            }
        };
        impl core::ops::Deref for TvEpisode {
            type Target = TvEpisodeReadOnly;
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self as *const Self as *const Self::Target) }
            }
        }
        #[repr(C)]
        pub struct TvEpisodeReadOnly {
            pub media: crate::entity::parent::ParentEntity<Media>,
            pub videos: crate::entity::field::Field<Vec<MediaVideo>>,
            pub belongs_to_season: crate::entity::field::Field<MediaId>,
            pub belongs_to_tv: crate::entity::field::Field<MediaId>,
            pub name: crate::entity::field::Field<MultiLangText>,
            pub episode_number: crate::entity::field::Field<EpisodeNumber>,
            pub duration: crate::entity::field::Field<Option<MediaDuration>>,
        }
        impl TvEpisode {
            pub fn read_only(&self) -> &TvEpisodeReadOnly {
                ::core::ops::Deref::deref(self)
            }
        }
        #[allow(unused)]
        pub use tv_episode_subsets::*;
        pub mod tv_episode_subsets {
            #![allow(unused_imports)]
            use super::*;
            pub struct TvEpisodeMini {
                pub media: MediaMini,
            }
            impl crate::entity::subset::Subset for TvEpisodeMini {
                type Entity = TvEpisode;

                fn to_entity(self) -> Self::Entity {
                    TvEpisode {
                        media: crate::entity::field::Unchanged::unchanged(<Media>::from(
                            self.media,
                        )),
                        videos: crate::entity::field::Unloaded::unloaded(),
                        belongs_to_season: crate::entity::field::Unloaded::unloaded(),
                        belongs_to_tv: crate::entity::field::Unloaded::unloaded(),
                        name: crate::entity::field::Unloaded::unloaded(),
                        episode_number: crate::entity::field::Unloaded::unloaded(),
                        duration: crate::entity::field::Unloaded::unloaded(),
                    }
                }
            }
            pub struct TvEpisode1 {
                pub media: MediaMini,
                pub episode_number: EpisodeNumber,
                pub videos: Vec<MediaVideo>,
            }
            impl crate::entity::subset::Subset for TvEpisode1 {
                type Entity = TvEpisode;
                fn to_entity(self) -> Self::Entity {
                    TvEpisode {
                        media: crate::entity::field::Unchanged::unchanged(<Media>::from(
                            self.media,
                        )),
                        episode_number: crate::entity::field::Unchanged::unchanged(
                            self.episode_number,
                        ),
                        videos: crate::entity::field::Unchanged::unchanged(self.videos),
                        belongs_to_season: crate::entity::field::Unloaded::unloaded(),
                        belongs_to_tv: crate::entity::field::Unloaded::unloaded(),
                        name: crate::entity::field::Unloaded::unloaded(),
                        duration: crate::entity::field::Unloaded::unloaded(),
                    }
                }
            }
        }
    }

    use movie::*;
    mod movie {
        #[allow(unused)]
        pub struct MovieModel {
            pub media: MediaModel,
            pub videos: Vec<MediaVideo>,
            pub duration: Option<MediaDuration>,
        }
        const _: () = {
            use crate::entity::model::Model;
            impl Model for MovieModel {
                type Entity = Movie;
                fn build_entity(self) -> Self::Entity {
                    Movie {
                        media: crate::entity::field::Reset::reset(self.media.build_entity()),
                        videos: crate::entity::field::Reset::reset(self.videos),
                        duration: crate::entity::field::Reset::reset(self.duration),
                    }
                }
            }
        };
        impl MovieModel {
            #[allow(unused)]
            pub fn build_entity(self) -> Movie {
                crate::entity::model::Model::build_entity(self)
            }
        }

        #[derive(Default)]
        pub struct MovieUpdater {
            pub media: MediaUpdater,
            pub duration: Option<Option<MediaDuration>>,
        }

        impl Updater for MovieUpdater {
            type GuardedStruct = Movie;
        }

        impl Movie {
            pub fn update(&mut self, updater: MovieUpdater) {
                self.media.update_fields(updater.media);
                self.duration.update_value(updater.duration);
            }
        }

        #[repr(C)]
        pub struct Movie {
            media: crate::entity::parent::ParentEntity<Media>,
            videos: crate::entity::field::Field<Vec<MediaVideo>>,
            duration: crate::entity::field::Field<Option<MediaDuration>>,
        }

        impl GuardedStruct for Movie {
            type Updater = MovieUpdater;

            fn update_fields(&mut self, updater: Self::Updater) {
                todo!()
            }
        }

        const _: () = {
            use crate::entity::Entity;
            impl Entity for Movie {
                type Id<'a> = <Media as Entity>::Id<'a>;

                type SysId = <Media as Entity>::SysId;
            }
        };

        impl core::ops::Deref for Movie {
            type Target = MovieReadOnly;
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self as *const Self as *const Self::Target) }
            }
        }
        #[repr(C)]
        pub struct MovieReadOnly {
            pub media: crate::entity::parent::ParentEntity<Media>,
            pub videos: crate::entity::field::Field<Vec<MediaVideo>>,
            pub duration: crate::entity::field::Field<Option<MediaDuration>>,
        }

        impl Movie {
            pub fn read_only(&self) -> &MovieReadOnly {
                ::core::ops::Deref::deref(self)
            }
        }
        #[allow(unused)]
        pub use movie_subsets::*;

        use crate::entity::{tests::Media, updater::Updater, GuardedStruct};

        use super::{MediaDuration, MediaModel, MediaUpdater, MediaVideo};
        pub mod movie_subsets {
            #![allow(unused_imports)]
            use crate::entity::tests::MediaMini;

            use super::*;

            pub struct MovieMini {
                pub media: MediaMini,
            }

            impl crate::entity::subset::Subset for MovieMini {
                type Entity = Movie;
                fn to_entity(self) -> Self::Entity {
                    Movie {
                        media: crate::entity::field::Unchanged::unchanged(<Media>::from(
                            self.media,
                        )),
                        videos: crate::entity::field::Unloaded::unloaded(),
                        duration: crate::entity::field::Unloaded::unloaded(),
                    }
                }
            }
        }
    }

    use album_def::*;

    use super::{updater::Updater, GuardedStruct};
    mod album_def {
        // Recursive expansion of FieldGuard macro
        // ========================================

        #[derive(Serialize, Deserialize, Default)]
        pub struct MediaAlbumModel {
            pub cover: Option<PictureId>,
            pub posters: Vec<PictureId>,
            pub backdrops: Vec<PictureId>,
            pub stills: Vec<PictureId>,
        }

        impl GuardedStruct for MediaAlbum {
            type Updater = MediaAlbumUpdater;

            fn update_fields(&mut self, updater: Self::Updater) {
                self.cover.update_value(updater.cover);
                self.posters.update_value(updater.posters);
                self.backdrops.update_value(updater.backdrops);
                self.stills.update_value(updater.stills);
            }
        }

        impl MediaAlbumModel {
            pub fn build(self) -> MediaAlbum {
                MediaAlbum {
                    cover: crate::entity::field::Reset::reset(self.cover),
                    posters: crate::entity::field::Reset::reset(self.posters),
                    backdrops: crate::entity::field::Reset::reset(self.backdrops),
                    stills: crate::entity::field::Reset::reset(self.stills),
                }
            }
        }
        impl From<MediaAlbumModel> for MediaAlbum {
            fn from(model: MediaAlbumModel) -> Self {
                model.build()
            }
        }
        #[derive(Serialize, Deserialize, Default)]
        pub struct MediaAlbumUpdater {
            pub cover: Option<Option<PictureId>>,
            pub posters: Option<Vec<PictureId>>,
            pub backdrops: Option<Vec<PictureId>>,
            pub stills: Option<Vec<PictureId>>,
        }

        impl Updater for MediaAlbumUpdater {
            type GuardedStruct = MediaAlbum;
        }

        impl MediaAlbum {
            pub fn update(&mut self, updater: MediaAlbumUpdater) {
                self.cover.update_value(updater.cover);
                self.posters.update_value(updater.posters);
                self.backdrops.update_value(updater.backdrops);
                self.stills.update_value(updater.stills);
            }
        }

        #[repr(C)]
        pub struct MediaAlbum {
            cover: crate::entity::field::Field<Option<PictureId>>,
            posters: crate::entity::field::Field<Vec<PictureId>>,
            backdrops: crate::entity::field::Field<Vec<PictureId>>,
            stills: crate::entity::field::Field<Vec<PictureId>>,
        }

        impl core::ops::Deref for MediaAlbum {
            type Target = MediaAlbumReadOnly;
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self as *const Self as *const Self::Target) }
            }
        }
        #[repr(C)]
        pub struct MediaAlbumReadOnly {
            pub cover: crate::entity::field::Field<Option<PictureId>>,
            pub posters: crate::entity::field::Field<Vec<PictureId>>,
            pub backdrops: crate::entity::field::Field<Vec<PictureId>>,
            pub stills: crate::entity::field::Field<Vec<PictureId>>,
        }

        impl crate::entity::field::Unloaded for MediaAlbum {
            fn unloaded() -> Self {
                Self {
                    cover: crate::entity::field::Unloaded::unloaded(),
                    posters: crate::entity::field::Unloaded::unloaded(),
                    backdrops: crate::entity::field::Unloaded::unloaded(),
                    stills: crate::entity::field::Unloaded::unloaded(),
                }
            }
        }

        impl MediaAlbum {
            pub fn read_only(&self) -> &MediaAlbumReadOnly {
                core::ops::Deref::deref(self)
            }
        }
        use serde::{Deserialize, Serialize};

        use crate::entity::{updater::Updater, GuardedStruct};

        use super::PictureId;
    }
}
