use crate::ldtk::{loaded_level::LoadedLevel, Level};
use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use thiserror::Error;

/// Secondary asset for loading external-levels ldtk files, specific to level data.
///
/// Loaded as a dependency of the [`LdtkProject`] asset.
///
/// Requires the `external_levels` feature to be enabled.
///
/// [`LdtkProject`]: crate::assets::LdtkProject
#[derive(Clone, Debug, PartialEq, TypeUuid, Reflect)]
#[uuid = "5448469b-2134-44f5-a86c-a7b829f70a0c"]
pub struct LdtkExternalLevel {
    /// Raw ldtk level data.
    data: Level,
}

impl LdtkExternalLevel {
    #[cfg(test)]
    pub fn new(data: Level) -> LdtkExternalLevel {
        LdtkExternalLevel { data }
    }

    pub fn data(&self) -> LoadedLevel {
        LoadedLevel::try_from(&self.data)
            .expect("construction of LdtkExternalLevel should guarantee that the level is loaded.")
    }

    pub fn background_image(&self) -> &Option<Handle<Image>> {
        &None
    }
}

#[derive(Debug, Error)]
pub enum LdtkExternalLevelLoaderError {
    #[error("external LDtk level should contain all level data, but the level's layers is null")]
    NullLayers,
}

#[derive(Default)]
pub struct LdtkExternalLevelLoader;

impl AssetLoader for LdtkExternalLevelLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            let data: Level = serde_json::from_slice(bytes)?;

            if data.layer_instances.is_none() {
                Err(LdtkExternalLevelLoaderError::NullLayers)?;
            }

            let ldtk_level = LdtkExternalLevel { data };

            let loaded_asset = LoadedAsset::new(ldtk_level);

            load_context.set_default_asset(loaded_asset);
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ldtkl"]
    }
}