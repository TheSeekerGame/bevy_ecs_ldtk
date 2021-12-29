//! Utility functions used internally by the plugin that have been exposed to the public api.

#[allow(unused_imports)]
use crate::components::*;

use crate::ldtk::*;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use std::{collections::HashMap, hash::Hash};

/// The `int_grid_csv` field of a [LayerInstance] is a 1-dimensional [Vec<i32>].
/// This function can map the indices of this [Vec] to a corresponding [TilePos].
///
/// Will return [None] if the resulting [TilePos] is out of the bounds implied by the width and
/// height.
pub fn int_grid_index_to_tile_pos(
    index: usize,
    layer_width_in_tiles: u32,
    layer_height_in_tiles: u32,
) -> Option<TilePos> {
    if layer_width_in_tiles * layer_height_in_tiles == 0 {
        // Checking for potential n mod 0 and n / 0 issues
        // Also it just doesn't make sense for either of these to be 0.
        return None;
    }

    let tile_x = index as u32 % layer_width_in_tiles;

    let inverted_y = (index as u32 - tile_x) / layer_width_in_tiles;

    if layer_height_in_tiles > inverted_y {
        // Checking for potential subtraction issues.
        // We don't need to check index >= tile_x because tile_x is defined as index mod n where n
        // is a natural number.
        // This means tile_x == index where index < n, and tile_x < index where index >= n.

        Some(ldtk_grid_coords_to_tile_pos(
            IVec2::new(tile_x as i32, inverted_y as i32),
            layer_height_in_tiles as i32,
        ))
    } else {
        None
    }
}

/// Simple conversion from a list of [EntityDefinition]s to a map using their Uids as the keys.
pub fn create_entity_definition_map(
    entity_definitions: &[EntityDefinition],
) -> HashMap<i32, &EntityDefinition> {
    entity_definitions.iter().map(|e| (e.uid, e)).collect()
}

/// Performs [EntityInstance] to [Transform] conversion
///
/// The `entity_definition_map` should be a map of [EntityDefinition] uids to [EntityDefinition]s.
///
/// Internally, this transform is used to place [EntityInstance]s, as children of the
/// [LdtkMapBundle].
pub fn calculate_transform_from_entity_instance(
    entity_instance: &EntityInstance,
    entity_definition_map: &HashMap<i32, &EntityDefinition>,
    level_height: i32,
    z_value: f32,
) -> Transform {
    let entity_definition = entity_definition_map.get(&entity_instance.def_uid).unwrap();

    let location = IVec2::from_slice(entity_instance.px.as_slice());

    let pivot = Vec2::from_slice(entity_instance.pivot.as_slice());

    let def_size = match &entity_instance.tile {
        Some(tile) => IVec2::new(tile.src_rect[2], tile.src_rect[3]),
        None => IVec2::new(entity_definition.width, entity_definition.height),
    };

    let size = IVec2::new(entity_instance.width, entity_instance.height);

    let translation =
        ldtk_pixel_coords_to_translation_pivoted(location, level_height as i32, size, pivot);
    let scale = size.as_vec2() / def_size.as_vec2();

    Transform::from_translation(translation.extend(z_value)).with_scale(scale.extend(1.))
}

/// Performs [TilePos] to [Transform] conversion
///
/// Note that the resulting Transform will be as if `TilePos(0, 0)` is at `(size / 2, size / 2,
/// z_value)`.
/// Internally, this transform is used to place [IntGridCell]s, as children of the [LdtkMapBundle].
pub fn calculate_transform_from_tile_pos(
    tile_pos: TilePos,
    tile_size: u32,
    z_value: f32,
) -> Transform {
    let tile_pos: UVec2 = tile_pos.into();
    let tile_size = Vec2::splat(tile_size as f32);
    let translation = tile_size * Vec2::splat(0.5) + tile_size * tile_pos.as_vec2();

    Transform::from_xyz(translation.x, translation.y, z_value)
}

fn ldtk_coord_conversion(coords: IVec2, height: i32) -> IVec2 {
    IVec2::new(coords.x, height - coords.y - 1)
}

pub fn ldtk_pixel_coords_to_translation(ldtk_coords: IVec2, ldtk_pixel_height: i32) -> Vec2 {
    ldtk_coord_conversion(ldtk_coords, ldtk_pixel_height).as_vec2()
}

pub fn translation_to_ldtk_pixel_coords(translation: Vec2, ldtk_pixel_height: i32) -> IVec2 {
    ldtk_coord_conversion(translation.as_ivec2(), ldtk_pixel_height)
}

pub fn ldtk_grid_coords_to_tile_pos(ldtk_coords: IVec2, ldtk_grid_height: i32) -> TilePos {
    let tile_coords = ldtk_coord_conversion(ldtk_coords, ldtk_grid_height).as_uvec2();
    TilePos(tile_coords.x, tile_coords.y)
}

pub fn tile_pos_to_ldtk_grid_coords(tile_pos: TilePos, ldtk_grid_height: i32) -> IVec2 {
    let tile_coords: UVec2 = tile_pos.into();
    ldtk_coord_conversion(tile_coords.as_ivec2(), ldtk_grid_height)
}

pub fn ldtk_grid_coords_to_translation(
    ldtk_coords: IVec2,
    ldtk_grid_height: i32,
    grid_size: IVec2,
) -> Vec2 {
    ldtk_pixel_coords_to_translation(ldtk_coords * grid_size, ldtk_grid_height * grid_size.y)
}

pub fn ldtk_grid_coords_to_translation_centered(
    ldtk_coords: IVec2,
    ldtk_grid_height: i32,
    grid_size: IVec2,
) -> Vec2 {
    ldtk_pixel_coords_to_translation(ldtk_coords * grid_size, ldtk_grid_height * grid_size.y)
        + Vec2::new(grid_size.x as f32, -grid_size.y as f32)
}

pub fn tile_pos_to_translation_centered(tile_pos: TilePos, tile_size: IVec2) -> Vec2 {
    let tile_coords: UVec2 = tile_pos.into();
    let tile_size = tile_size.as_vec2();
    (tile_size * tile_coords.as_vec2()) + (tile_size / Vec2::splat(0.5))
}

pub fn ldtk_pixel_coords_to_translation_pivoted(
    ldtk_coords: IVec2,
    ldtk_pixel_height: i32,
    grid_size: IVec2,
    pivot: Vec2,
) -> Vec2 {
    let pivot_point = ldtk_coord_conversion(ldtk_coords, ldtk_pixel_height).as_vec2();

    let adjusted_pivot = Vec2::new(0.5 - pivot.x, pivot.y - 0.5);

    let offset = grid_size.as_vec2() * adjusted_pivot;

    pivot_point + offset
}

/// Similar to [LayerBuilder::new_batch], except it doesn't consume the [LayerBuilder]
///
/// This allows for more methods to be performed on the [LayerBuilder] before building it.
/// However, the performance cons of using non-batch methods still apply here.
pub fn set_all_tiles_with_func<T>(
    layer_builder: &mut LayerBuilder<T>,
    mut func: impl FnMut(TilePos) -> Option<T>,
) where
    T: TileBundleTrait,
{
    let map_size: Vec2 = layer_builder.settings.map_size.into();
    let chunk_size: Vec2 = layer_builder.settings.chunk_size.into();
    let map_size_in_tiles = (map_size * chunk_size).as_uvec2();
    for x in 0..map_size_in_tiles.x {
        for y in 0..map_size_in_tiles.y {
            let tile_pos = TilePos(x, y);
            if let Some(t) = func(tile_pos) {
                layer_builder.set_tile(tile_pos, t).unwrap()
            }
        }
    }
}

/// Wraps `a` and `b` in an [Option] and tries each [Some]/[None] permutation as inputs to `func`,
/// returning the first non-none result of `func`.
///
/// The permutations are tried in this order:
/// 1. Some, Some
/// 2. None, Some
/// 3. Some, None
/// 4. None, None
///
/// Used for the defaulting functionality of [bevy_ecs_ldtk::app::RegisterLdtkObjects]
pub(crate) fn try_each_optional_permutation<A, B, R>(
    a: A,
    b: B,
    mut func: impl FnMut(Option<A>, Option<B>) -> Option<R>,
) -> Option<R>
where
    A: Clone,
    B: Clone,
{
    func(Some(a.clone()), Some(b.clone()))
        .or_else(|| func(None, Some(b)))
        .or_else(|| func(Some(a), None))
        .or_else(|| func(None, None))
}

/// The "get" function used on [bevy_ecs_ldtk::app::LdtkEntityMap] and
/// [bevy_ecs_ldtk::app::LdtkIntCellMap].
///
/// Due to the defaulting functionality of [bevy_ecs_ldtk::app::RegisterLdtkObjects], a single
/// instance of an LDtk entity or int grid tile may match multiple registrations.
/// This function is responsible for picking the correct registration while spawning these
/// entities/tiles.
pub(crate) fn ldtk_map_get_or_default<'a, A, B, L>(
    a: A,
    b: B,
    default: &'a L,
    map: &'a HashMap<(Option<A>, Option<B>), L>,
) -> &'a L
where
    A: Hash + Eq + Clone,
    B: Hash + Eq + Clone,
{
    try_each_optional_permutation(a, b, |x, y| map.get(&(x, y))).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_grid_index_to_tile_pos() {
        assert_eq!(int_grid_index_to_tile_pos(3, 4, 5), Some(TilePos(3, 4)));

        assert_eq!(int_grid_index_to_tile_pos(10, 5, 5), Some(TilePos(0, 2)));

        assert_eq!(int_grid_index_to_tile_pos(49, 10, 5), Some(TilePos(9, 0)));

        assert_eq!(int_grid_index_to_tile_pos(64, 100, 1), Some(TilePos(64, 0)));

        assert_eq!(int_grid_index_to_tile_pos(35, 1, 100), Some(TilePos(0, 64)));
    }

    #[test]
    fn test_int_grid_index_out_of_range() {
        assert_eq!(int_grid_index_to_tile_pos(3, 0, 5), None);

        assert_eq!(int_grid_index_to_tile_pos(3, 5, 0), None);

        assert_eq!(int_grid_index_to_tile_pos(25, 5, 5), None);
    }

    #[test]
    fn test_calculate_transform_from_entity_instance() {
        let entity_definitions = vec![
            EntityDefinition {
                uid: 0,
                width: 32,
                height: 32,
                ..Default::default()
            },
            EntityDefinition {
                uid: 1,
                width: 64,
                height: 16,
                ..Default::default()
            },
            EntityDefinition {
                uid: 2,
                width: 10,
                height: 25,
                ..Default::default()
            },
        ];
        let entity_definition_map = create_entity_definition_map(&entity_definitions);

        // simple case
        let entity_instance = EntityInstance {
            px: vec![256, 256],
            def_uid: 0,
            width: 32,
            height: 32,
            pivot: vec![0., 0.],
            ..Default::default()
        };
        let result = calculate_transform_from_entity_instance(
            &entity_instance,
            &entity_definition_map,
            320,
            0.,
        );
        assert_eq!(result, Transform::from_xyz(272., 47., 0.));

        // difficult case
        let entity_instance = EntityInstance {
            px: vec![40, 50],
            def_uid: 2,
            width: 30,
            height: 50,
            pivot: vec![1., 1.],
            ..Default::default()
        };
        let result = calculate_transform_from_entity_instance(
            &entity_instance,
            &entity_definition_map,
            100,
            2.,
        );
        assert_eq!(
            result,
            Transform::from_xyz(25., 74., 2.).with_scale(Vec3::new(3., 2., 1.))
        );
    }

    #[test]
    fn test_calculate_transform_from_entity_instance_with_tile() {
        let entity_definitions = vec![EntityDefinition {
            uid: 0,
            width: 32,
            height: 32,
            ..Default::default()
        }];
        let entity_definition_map = create_entity_definition_map(&entity_definitions);

        let entity_instance = EntityInstance {
            px: vec![64, 64],
            def_uid: 0,
            width: 64,
            height: 64,
            pivot: vec![1., 1.],
            tile: Some(EntityInstanceTile {
                src_rect: vec![0, 0, 16, 32],
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = calculate_transform_from_entity_instance(
            &entity_instance,
            &entity_definition_map,
            100,
            2.,
        );
        assert_eq!(
            result,
            Transform::from_xyz(32., 67., 2.).with_scale(Vec3::new(4., 2., 1.))
        );
    }

    #[test]
    fn test_calculate_transform_from_tile_pos() {
        assert_eq!(
            calculate_transform_from_tile_pos(TilePos(1, 2), 32, 0.),
            Transform::from_xyz(48., 80., 0.)
        );

        assert_eq!(
            calculate_transform_from_tile_pos(TilePos(1, 0), 100, 50.),
            Transform::from_xyz(150., 50., 50.)
        );

        assert_eq!(
            calculate_transform_from_tile_pos(TilePos(0, 5), 1, 1.),
            Transform::from_xyz(0.5, 5.5, 1.)
        );
    }

    #[test]
    fn test_try_each_optional_permutation() {
        fn test_func(a: Option<i32>, b: Option<i32>) -> Option<i32> {
            match (a, b) {
                (Some(a), Some(_)) if a == 1 => Some(1),
                (Some(_), Some(_)) => None,
                (Some(a), None) if a == 2 => Some(2),
                (Some(_), None) => None,
                (None, Some(b)) if b == 3 => Some(3),
                (None, Some(_)) => None,
                (None, None) => Some(4),
            }
        }

        assert_eq!(try_each_optional_permutation(1, 1, test_func), Some(1));
        assert_eq!(try_each_optional_permutation(2, 1, test_func), Some(2));
        assert_eq!(try_each_optional_permutation(2, 2, test_func), Some(2));
        assert_eq!(try_each_optional_permutation(2, 3, test_func), Some(3));
        assert_eq!(try_each_optional_permutation(3, 3, test_func), Some(3));
        assert_eq!(try_each_optional_permutation(4, 3, test_func), Some(3));
        assert_eq!(try_each_optional_permutation(4, 4, test_func), Some(4));
        assert_eq!(try_each_optional_permutation(5, 5, test_func), Some(4));
    }
}
