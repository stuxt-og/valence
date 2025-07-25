#![doc = include_str!("../README.md")]
#![allow(clippy::unseparated_literal_suffix, clippy::manual_string_new)]

pub mod active_status_effects;
pub mod attributes;
mod flags;
pub mod hitbox;
pub mod manager;
pub mod query;
pub mod tracked_data;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use derive_more::{Deref, DerefMut};
pub use manager::EntityManager;
use paste::paste;
use tracing::warn;
use tracked_data::TrackedData;
use valence_math::{DVec3, Vec3};
use valence_protocol::{decode, Decode, Encode, Ident, VarInt};
use valence_server_common::{Despawned, UniqueId};
use valence_protocol::registry::RegistryEntry;
use valence_ident::Ident;
use crate::attributes::TrackedEntityAttributes;

include!(concat!(env!("OUT_DIR"), "/entity.rs"));

pub struct EntityPlugin;

/// When new Minecraft entities are initialized and added to
/// [`EntityManager`].
///
/// Systems that need Minecraft entities to be in a valid state should run
/// _after_ this set.
///
/// This set lives in [`PostUpdate`].
#[derive(SystemSet, Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct InitEntitiesSet;

/// When tracked data is written to the entity's [`TrackedData`] component.
/// Systems that modify tracked data should run _before_ this.
///
/// This set lives in [`PostUpdate`].
#[derive(SystemSet, Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct UpdateTrackedDataSet;

/// When entities are updated and changes from the current tick are cleared.
/// Systems that need to observe changes to entities (Such as the difference
/// between [`Position`] and [`OldPosition`]) should run _before_ this set (and
/// probably after [`InitEntitiesSet`]).
///
/// This set lives in [`PostUpdate`].
#[derive(SystemSet, Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ClearEntityChangesSet;

impl Plugin for EntityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EntityManager::new())
            .configure_sets(
                PostUpdate,
                (
                    InitEntitiesSet,
                    UpdateTrackedDataSet,
                    ClearEntityChangesSet
                        .after(InitEntitiesSet)
                        .after(UpdateTrackedDataSet),
                ),
            )
            .add_systems(
                PostUpdate,
                (remove_despawned_from_manager, init_entities)
                    .chain()
                    .in_set(InitEntitiesSet),
            )
            .add_systems(
                PostUpdate,
                (
                    clear_status_changes,
                    clear_animation_changes,
                    clear_tracked_data_changes,
                    clear_tracked_attributes_changes,
                    update_old_position,
                    update_old_layer_id,
                )
                    .in_set(ClearEntityChangesSet),
            );

        add_tracked_data_systems(app);
    }
}

fn update_old_position(mut query: Query<(&Position, &mut OldPosition)>) {
    for (pos, mut old_pos) in &mut query {
        old_pos.0 = pos.0;
    }
}

fn update_old_layer_id(mut query: Query<(&EntityLayerId, &mut OldEntityLayerId)>) {
    for (loc, mut old_loc) in &mut query {
        old_loc.0 = loc.0;
    }
}

fn remove_despawned_from_manager(
    entities: Query<&EntityId, (With<EntityKind>, With<Despawned>)>,
    mut manager: ResMut<EntityManager>,
) {
    for id in &entities {
        manager.id_to_entity.remove(&id.0);
    }
}

fn init_entities(
    mut entities: Query<
        (Entity, &mut EntityId, &Position, &mut OldPosition),
        (Added<EntityKind>, Without<Despawned>),
    >,
    mut manager: ResMut<EntityManager>,
) {
    for (entity, mut id, pos, mut old_pos) in &mut entities {
        *old_pos = OldPosition::new(pos.0);

        if *id == EntityId::default() {
            *id = manager.next_id();
        }

        if let Some(conflict) = manager.id_to_entity.insert(id.0, entity) {
            warn!(
                "entity {entity:?} has conflicting entity ID of {} with entity {conflict:?}",
                id.0
            );
        }
    }
}

fn clear_status_changes(mut statuses: Query<&mut EntityStatuses, Changed<EntityStatuses>>) {
    for mut statuses in &mut statuses {
        statuses.0 = 0;
    }
}

fn clear_animation_changes(
    mut animations: Query<&mut EntityAnimations, Changed<EntityAnimations>>,
) {
    for mut animations in &mut animations {
        animations.0 = 0;
    }
}

fn clear_tracked_data_changes(mut tracked_data: Query<&mut TrackedData, Changed<TrackedData>>) {
    for mut tracked_data in &mut tracked_data {
        tracked_data.clear_update_values();
    }
}

fn clear_tracked_attributes_changes(
    mut attributes: Query<&mut TrackedEntityAttributes, Changed<TrackedEntityAttributes>>,
) {
    for mut attributes in &mut attributes {
        attributes.clear();
    }
}

/// Contains the entity layer an entity is on.
#[derive(Component, Copy, Clone, PartialEq, Eq, Debug, Deref)]
pub struct EntityLayerId(pub Entity);

impl Default for EntityLayerId {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

impl PartialEq<OldEntityLayerId> for EntityLayerId {
    fn eq(&self, other: &OldEntityLayerId) -> bool {
        self.0 == other.0
    }
}

/// The value of [`EntityLayerId`] from the end of the previous tick.
#[derive(Component, Copy, Clone, PartialEq, Eq, Debug, Deref)]
pub struct OldEntityLayerId(Entity);

impl OldEntityLayerId {
    pub fn get(&self) -> Entity {
        self.0
    }
}

impl Default for OldEntityLayerId {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

impl PartialEq<EntityLayerId> for OldEntityLayerId {
    fn eq(&self, other: &EntityLayerId) -> bool {
        self.0 == other.0
    }
}

#[derive(Component, Copy, Clone, PartialEq, Default, Debug, Deref, DerefMut)]
pub struct Position(pub DVec3);

impl Position {
    pub fn new<P: Into<DVec3>>(pos: P) -> Self {
        Self(pos.into())
    }

    pub fn get(self) -> DVec3 {
        self.0
    }

    pub fn set<P: Into<DVec3>>(&mut self, pos: P) {
        self.0 = pos.into();
    }
}

impl PartialEq<OldPosition> for Position {
    fn eq(&self, other: &OldPosition) -> bool {
        self.0 == other.0
    }
}

/// The value of [`Position`] from the end of the previous tick.
///
/// **NOTE**: You should not modify this component after the entity is spawned.
#[derive(Component, Clone, PartialEq, Default, Debug, Deref)]
pub struct OldPosition(DVec3);

impl OldPosition {
    pub fn new<P: Into<DVec3>>(pos: P) -> Self {
        Self(pos.into())
    }

    pub fn get(&self) -> DVec3 {
        self.0
    }
}

impl PartialEq<Position> for OldPosition {
    fn eq(&self, other: &Position) -> bool {
        self.0 == other.0
    }
}

/// Describes the direction an entity is looking using pitch and yaw angles.
#[derive(Component, Copy, Clone, PartialEq, Default, Debug)]
pub struct Look {
    /// The yaw angle in degrees, where:
    /// - `-90` is looking east (towards positive x).
    /// - `0` is looking south (towards positive z).
    /// - `90` is looking west (towards negative x).
    /// - `180` is looking north (towards negative z).
    ///
    /// Values -180 to 180 are also valid.
    pub yaw: f32,
    /// The pitch angle in degrees, where:
    /// - `-90` is looking straight up.
    /// - `0` is looking straight ahead.
    /// - `90` is looking straight down.
    pub pitch: f32,
}

impl Look {
    pub const fn new(yaw: f32, pitch: f32) -> Self {
        Self { yaw, pitch }
    }

    /// Gets a normalized direction vector from the yaw and pitch.
    pub fn vec(self) -> Vec3 {
        let (yaw_sin, yaw_cos) = (self.yaw + 90.0).to_radians().sin_cos();
        let (pitch_sin, pitch_cos) = (-self.pitch).to_radians().sin_cos();

        Vec3::new(yaw_cos * pitch_cos, pitch_sin, yaw_sin * pitch_cos)
    }

    /// Sets the yaw and pitch using a normalized direction vector.
    pub fn set_vec(&mut self, dir: Vec3) {
        debug_assert!(
            dir.is_normalized(),
            "the direction vector should be normalized"
        );

        // Preserve the current yaw if we're looking straight up or down.
        if dir.x != 0.0 || dir.z != 0.0 {
            self.yaw = f32::atan2(dir.z, dir.x).to_degrees() - 90.0;
        }

        self.pitch = -(dir.y).asin().to_degrees();
    }
}

#[derive(Component, Copy, Clone, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct OnGround(pub bool);

/// A Minecraft entity's ID according to the protocol.
///
/// IDs should be _unique_ for the duration of the server and  _constant_ for
/// the lifetime of the entity. IDs of -1 (the default) will be assigned to
/// something else on the tick the entity is added. If you need to know the ID
/// ahead of time, set this component to the value returned by
/// [`EntityManager::next_id`] before spawning.
#[derive(Component, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deref)]
pub struct EntityId(i32);

impl EntityId {
    /// Returns the underlying entity ID as an integer.
    pub fn get(self) -> i32 {
        self.0
    }
}

/// Returns an entity ID of -1.
impl Default for EntityId {
    fn default() -> Self {
        Self(-1)
    }
}

#[derive(Component, Copy, Clone, PartialEq, Default, Debug, Deref, DerefMut)]
pub struct HeadYaw(pub f32);

/// Entity velocity in m/s.
#[derive(Component, Copy, Clone, Default, Debug, Deref, DerefMut)]
pub struct Velocity(pub Vec3);

impl Velocity {
    pub fn to_packet_units(self) -> valence_protocol::Velocity {
        valence_protocol::Velocity::from_ms_f32(self.0.into())
    }
}

// TODO: don't make statuses and animations components.

#[derive(Component, Copy, Clone, Default, Debug, Deref, DerefMut)]
pub struct EntityStatuses(pub u64);

impl EntityStatuses {
    pub fn trigger(&mut self, status: EntityStatus) {
        self.set(status, true);
    }

    pub fn set(&mut self, status: EntityStatus, triggered: bool) {
        self.0 |= u64::from(triggered) << status as u64;
    }

    pub fn get(&self, status: EntityStatus) -> bool {
        (self.0 >> status as u64) & 1 == 1
    }
}

#[derive(Component, Default, Debug, Copy, Clone, Deref, DerefMut)]
pub struct EntityAnimations(pub u8);

impl EntityAnimations {
    pub fn trigger(&mut self, anim: EntityAnimation) {
        self.set(anim, true);
    }

    pub fn set(&mut self, anim: EntityAnimation, triggered: bool) {
        self.0 |= u8::from(triggered) << anim as u8;
    }

    pub fn get(&self, anim: EntityAnimation) -> bool {
        (self.0 >> anim as u8) & 1 == 1
    }
}

/// Extra integer data passed to the entity spawn packet. The meaning depends on
/// the type of entity being spawned.
///
/// Some examples:
/// - **Experience Orb**: Experience count
/// - **(Glowing) Item Frame**: Rotation
/// - **Painting**: Rotation
/// - **Falling Block**: Block state
/// - **Fishing Bobber**: Hook entity ID
/// - **Warden**: Initial pose
#[derive(Component, Default, Debug, Deref, DerefMut)]
pub struct ObjectData(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct VillagerData {
    pub kind: VillagerKind,
    pub profession: VillagerProfession,
    pub level: i32,
}

impl VillagerData {
    pub const fn new(kind: VillagerKind, profession: VillagerProfession, level: i32) -> Self {
        Self {
            kind,
            profession,
            level,
        }
    }
}

impl Default for VillagerData {
    fn default() -> Self {
        Self {
            kind: Default::default(),
            profession: Default::default(),
            level: 1,
        }
    }
}

impl Encode for VillagerData {
    fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
        self.kind.encode(&mut w)?;
        self.profession.encode(&mut w)?;
        VarInt(self.level).encode(w)
    }
}

impl Decode<'_> for VillagerData {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Self {
            kind: VillagerKind::decode(r)?,
            profession: VillagerProfession::decode(r)?,
            level: VarInt::decode(r)?.0,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Encode, Decode)]
pub enum VillagerKind {
    Desert,
    Jungle,
    #[default]
    Plains,
    Savanna,
    Snow,
    Swamp,
    Taiga,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Encode, Decode)]
pub enum VillagerProfession {
    #[default]
    None,
    Armorer,
    Butcher,
    Cartographer,
    Cleric,
    Farmer,
    Fisherman,
    Fletcher,
    Leatherworker,
    Librarian,
    Mason,
    Nitwit,
    Shepherd,
    Toolsmith,
    Weaponsmith,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Encode, Decode)]
pub enum Pose {
    #[default]
    Standing,
    FallFlying,
    Sleeping,
    Swimming,
    SpinAttack,
    Sneaking,
    LongJumping,
    Dying,
    Croaking,
    UsingTongue,
    Roaring,
    Sniffing,
    Emerging,
    Digging,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Encode, Decode)]
pub enum BoatKind {
    #[default]
    Oak,
    Spruce,
    Birch,
    Jungle,
    Acacia,
    DarkOak,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Encode, Decode)]
pub enum CatKind {
    Tabby,
    #[default]
    Black,
    Red,
    Siamese,
    BritishShorthair,
    Calico,
    Persian,
    Ragdoll,
    White,
    Jellie,
    AllBlack,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Encode, Decode)]
pub enum FrogKind {
    #[default]
    Temperate,
    Warm,
    Cold,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Encode, Decode)]
pub enum PaintingKind {
    #[default]
    Kebab,
    Aztec,
    Alban,
    Aztec2,
    Bomb,
    Plant,
    Wasteland,
    Pool,
    Courbet,
    Sea,
    Sunset,
    Creebet,
    Wanderer,
    Graham,
    Match,
    Bust,
    Stage,
    Void,
    SkullAndRoses,
    Wither,
    Fighters,
    Pointer,
    Pigscene,
    BurningSkull,
    Skeleton,
    Earth,
    Wind,
    Water,
    Fire,
    DonkeyKong,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Encode, Decode)]
pub enum SnifferState {
    #[default]
    Idling,
    FeelingHappy,
    Scenting,
    Sniffing,
    Searching,
    Digging,
    Rising,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Encode, Decode)]
pub enum ArmadilloState {
    #[default]
    Idle,
    Rolling,
    Scared,
}

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WolfVariant {
    pub wild_texture: Ident,
    pub tame_texture: Ident,
    pub angry_texture: Ident,
    pub biomes: Vec<RegistryEntry<Biome>>,
}

impl WolfVariant {
    pub const fn new(wild_texture: Ident, tame_texture: Ident, angry_texture: Ident, biomes: Vec<RegistryEntry<Biome>>) -> Self {
        Self {
            wild_texture,
            tame_texture,
            angry_texture,
            biomes,
        }
    }
}

impl Default for WolfVariant {
    fn default() -> Self {
        Self {
            wild_texture: Default::default(),
            tame_texture: Default::default(),
            angry_texture: Default::default(),
            biomes: Default::default(),
        }
    }
}

impl Encode for WolfVariant {
    fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
        self.wild_texture.encode(&mut w)?;
        self.tame_texture.encode(&mut w)?;
        self.angry_texture.encode(&mut w)?;
        self.biomes.encode(&mut w)
    }
}

impl Decode<'_> for WolfVariant {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Self {
            wild_texture: Ident::decode(r)?,
            tame_texture: Ident::decode(r)?,
            angry_texture: Ident::decode(r)?,
            biomes: Vec<RegistryEntry<Biome>>::decode(r)?,
        })
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Encode, Decode)]
pub struct EulerAngle {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

#[derive(Copy, Clone)]
struct OptionalInt(Option<i32>);

impl Encode for OptionalInt {
    fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
        if let Some(n) = self.0 {
            VarInt(n.wrapping_add(1))
        } else {
            VarInt(0)
        }
        .encode(w)
    }
}

impl Decode<'_> for OptionalInt {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let n = VarInt::decode(r)?.0;

        Ok(Self(if n == 0 {
            None
        } else {
            Some(n.wrapping_sub(1))
        }))
    }
}
