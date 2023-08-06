use crate::{
    assets::level_map::LevelMetadata,
    ldtk::{LdtkJson, Level},
    resources::LevelSelection,
};
use bevy::{
    asset::{AssetLoader, AssetPath, LoadContext, LoadedAsset},
    prelude::*,
    reflect::{TypePath, TypeUuid},
    utils::BoxedFuture,
};
use derive_getters::Getters;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

use super::level_map::LevelIndices;

fn ldtk_path_to_asset_path<'b>(ldtk_path: &Path, rel_path: &str) -> AssetPath<'b> {
    ldtk_path.parent().unwrap().join(Path::new(rel_path)).into()
}

/// Main asset for loading ldtk files.
///
/// Load your ldtk project with the asset server, then insert the handle into the
/// [`LdtkWorldBundle`].
///
/// [`LdtkWorldBundle`]: crate::components::LdtkWorldBundle
#[derive(Clone, Debug, PartialEq, TypeUuid, TypePath, Getters)]
#[uuid = "ecfb87b7-9cd9-4970-8482-f2f68b770d31"]
pub struct LdtkProject {
    /// Raw ldtk project data.
    data: LdtkJson,
    /// Map from tileset uids to image handles for the loaded tileset.
    tileset_map: HashMap<i32, Handle<Image>>,
    /// Map from level iids to level metadata.
    level_map: IndexMap<String, LevelMetadata>,
    /// Image used for rendering int grid colors.
    int_grid_image_handle: Option<Handle<Image>>,
}

impl LdtkProject {
    /// Get an iterator of all the levels in the LDtk file.
    ///
    /// This abstraction avoids compatibility issues between pre-multi-world and post-multi-world
    /// LDtk projects.
    ///
    /// Note: the returned levels are the ones existent in the [`LdtkProject`].
    /// These levels will have "incomplete" data if you use LDtk's external levels feature.
    /// To always get full level data, you'll need to access `Assets<LdtkLevel>`.
    pub fn iter_raw_levels(&self) -> impl Iterator<Item = &Level> {
        self.data.iter_raw_levels()
    }

    pub fn get_raw_level_by_indices(&self, indices: &LevelIndices) -> Option<&Level> {
        match indices.world_index() {
            Some(world_index) => self
                .data
                .worlds
                .get(*world_index)
                .and_then(|world| world.levels.get(*indices.level_index())),
            None => self.data.levels.get(*indices.level_index()),
        }
    }

    pub fn get_raw_level_by_iid(&self, iid: &String) -> Option<&Level> {
        self.level_map
            .get(iid)
            .and_then(|level_metadata| self.get_raw_level_by_indices(level_metadata.indices()))
    }

    pub fn get_raw_level_by_index(&self, index: usize) -> Option<&Level> {
        self.level_map
            .get_index(index)
            .and_then(|(_, level_metadata)| self.get_raw_level_by_indices(level_metadata.indices()))
    }

    /// Find a particular level using a [`LevelSelection`].
    ///
    /// Note: the returned level is the one existent in the [`LdtkProject`].
    /// This level will have "incomplete" data if you use LDtk's external levels feature.
    /// To always get full level data, you'll need to access `Assets<LdtkLevel>`.
    pub fn find_raw_level_by_level_selection(
        &self,
        level_selection: &LevelSelection,
    ) -> Option<&Level> {
        match level_selection {
            LevelSelection::Iid(iid) => self.get_raw_level_by_iid(iid),
            LevelSelection::Index(index) => self.get_raw_level_by_index(*index),
            _ => self
                .iter_raw_levels()
                .enumerate()
                .find(|(i, l)| level_selection.is_match(i, l))
                .map(|(_, l)| l),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum LdtkProjectLoaderError {
    #[error("external_levels feature enabled, but LDtk project uses internal levels")]
    InternalLevelProject,
    #[error("LDtk project uses external levels, but external_levels feature not enabled")]
    ExternalLevelProject,
    #[error("LDtk project uses internal levels, but the level's layers is null")]
    InternalLevelWithNullLayers,
}

#[derive(Default)]
pub struct LdtkProjectLoader;

struct LoadLevelMetadataResult<'a> {
    bg_image_path: Option<AssetPath<'a>>,
    external_level_path: Option<AssetPath<'a>>,
    level_metadata: LevelMetadata,
}

fn load_level_metadata<'a>(
    load_context: &LoadContext,
    level_indices: LevelIndices,
    level: &Level,
) -> Result<LoadLevelMetadataResult<'a>, LdtkProjectLoaderError> {
    let (bg_image_path, bg_image) = level
        .bg_rel_path
        .as_ref()
        .map(|rel_path| {
            let asset_path = ldtk_path_to_asset_path(load_context.path(), &rel_path);

            (
                Some(asset_path.clone()),
                Some(load_context.get_handle(asset_path)),
            )
        })
        .unwrap_or((None, None));

    #[cfg(feature = "external_levels")]
    {
        let external_level_path = ldtk_path_to_asset_path(
            load_context.path(),
            level.external_rel_path.as_ref().expect("TODO"),
        );

        let external_handle = load_context.get_handle(external_level_path.clone());

        let level_metadata = LevelMetadata::new(bg_image, level_indices, external_handle);

        Ok(LoadLevelMetadataResult {
            bg_image_path,
            level_metadata,
            external_level_path: Some(external_level_path),
        })
    }

    #[cfg(not(feature = "external_levels"))]
    {
        if level.layer_instances.is_none() {
            Err(LdtkProjectLoaderError::InternalLevelWithNullLayers)?;
        }

        let level_metadata = LevelMetadata::new(bg_image, level_indices);

        Ok(LoadLevelMetadataResult {
            bg_image_path,
            level_metadata,
            external_level_path: None,
        })
    }
}

impl AssetLoader for LdtkProjectLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            let data: LdtkJson = serde_json::from_slice(bytes)?;

            if data.external_levels && !cfg!(feature = "external_levels") {
                Err(LdtkProjectLoaderError::ExternalLevelProject)?;
            } else if !data.external_levels && cfg!(feature = "external_levels") {
                Err(LdtkProjectLoaderError::InternalLevelProject)?;
            }

            let mut level_map = IndexMap::new();

            let mut background_images = Vec::new();
            #[allow(unused_mut)]
            let mut external_level_paths = Vec::new();

            #[allow(unused_variables)]
            for (level_index, level) in data.levels.iter().enumerate() {
                let LoadLevelMetadataResult {
                    bg_image_path,
                    external_level_path,
                    level_metadata,
                } = load_level_metadata(load_context, LevelIndices::new(None, level_index), level)?;

                if let Some(bg_image_path) = bg_image_path {
                    background_images.push(bg_image_path);
                }
                if let Some(external_level_path) = external_level_path {
                    external_level_paths.push(external_level_path);
                }
                level_map.insert(level.iid.clone(), level_metadata);
            }

            let mut tileset_rel_paths = Vec::new();
            let mut tileset_map = HashMap::new();
            for tileset in &data.defs.tilesets {
                if let Some(tileset_path) = &tileset.rel_path {
                    let asset_path = ldtk_path_to_asset_path(load_context.path(), tileset_path);

                    tileset_rel_paths.push(asset_path.clone());
                    tileset_map.insert(tileset.uid, load_context.get_handle(asset_path));
                } else if tileset.embed_atlas.is_some() {
                    warn!("Ignoring LDtk's Internal_Icons. They cannot be displayed due to their license.");
                } else {
                    let identifier = &tileset.identifier;
                    warn!("{identifier} tileset cannot be loaded, it has a null relative path.");
                }
            }

            let int_grid_image_handle = data.defs.create_int_grid_image().map(|image| {
                load_context.set_labeled_asset("int_grid_image", LoadedAsset::new(image))
            });

            let ldtk_asset = LdtkProject {
                data,
                tileset_map,
                level_map,
                int_grid_image_handle,
            };

            load_context.set_default_asset(
                LoadedAsset::new(ldtk_asset)
                    .with_dependencies(tileset_rel_paths)
                    .with_dependencies(external_level_paths)
                    .with_dependencies(background_images),
            );
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ldtk"]
    }
}