use anyhow::Ok;
use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::quote;
use serde::Deserialize;
use valence_build_utils::{ident, rerun_if_changed};

use std::sync::atomic::{AtomicU32, Ordering};

static ID_COUNTER: AtomicU32 = AtomicU32::new(0);

fn next_id() -> u32 {
    ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

static ID_COUNTER: AtomicU32 = AtomicU32::new(0);

fn next_id() -> u32 {
    ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Deserialize, Clone, Debug)]
struct Item {
    id: u32,
    name: String,
    translation_key: String,
    max_stack: i8,
    max_durability: u16,
    enchantability: u8,
    fireproof: bool,
    food: Option<FoodComponent>,
}

#[derive(Deserialize, Clone, Debug)]
struct FoodComponent {
    hunger: u16,
    saturation: f32,
    always_edible: bool,
    meat: bool,
    snack: bool,
    // TODO: effects
}

pub(crate) fn build() -> anyhow::Result<TokenStream> {
    rerun_if_changed(["extracted/items.json"]);

    let items = serde_json::from_str::<Vec<Item>>(include_str!("../extracted/items.json"))?;

    let item_kind_count = items.len();

    let item_kind_from_raw_id_arms = items
        .iter()
        .map(|item| {
            let id = next_id();
            let name = ident(item.name.to_pascal_case());

            quote! {
                #id => Some(Self::#name),
            }
        })
        .collect::<TokenStream>();

        let item_kind_to_raw_id_arms = items
            .iter()
            .map(|item| {
                let id = next_id();
                let name = ident(item.name.to_pascal_case());

                quote! {
                    Self::#name => #id,
                }
            })
            .collect::<TokenStream>();

        let item_kind_from_str_arms = items
            .iter()
            .map(|item| {
                let str_name = &item.name;
                let name = ident(str_name.to_pascal_case());
                quote! {
                    #str_name => Some(Self::#name),
                }
            })
            .collect::<TokenStream>();

        let item_kind_to_str_arms = items
            .iter()
            .map(|item| {
                let str_name = &item.name;
                let name = ident(str_name.to_pascal_case());
                quote! {
                    Self::#name => #str_name,
                }
            })
            .collect::<TokenStream>();

        let item_kind_to_translation_key_arms = items
            .iter()
            .map(|item| {
                let name = ident(item.name.to_pascal_case());
                let translation_key = &item.translation_key;
                quote! {
                    Self::#name => #translation_key,
                }
            })
            .collect::<TokenStream>();

        let item_kind_variants = items
            .iter()
            .map(|item| ident(item.name.to_pascal_case()))
            .collect::<Vec<_>>();

        let item_kind_to_max_stack_arms = items
            .iter()
            .map(|item| {
                let name = ident(item.name.to_pascal_case());
                let max_stack = item.max_stack;

                quote! {
                    Self::#name => #max_stack,
                }
            })
            .collect::<TokenStream>();

        let item_kind_to_food_component_arms = items
            .iter()
            .map(|item| match &item.food {
                Some(food_component) => {
                    let name = ident(item.name.to_pascal_case());
                    let hunger = food_component.hunger;
                    let saturation = food_component.saturation;
                    let always_edible = food_component.always_edible;
                    let meat = food_component.meat;
                    let snack = food_component.snack;

                    quote! {
                        Self::#name => Some(FoodComponent {
                            hunger: #hunger,
                            saturation: #saturation,
                            always_edible: #always_edible,
                            meat: #meat,
                            snack: #snack,
                        }
                    ),
                    }
                }
                None => quote! {},
            })
            .collect::<TokenStream>();

        let item_kind_to_max_durability_arms = items
            .iter()
            .filter(|item| item.max_durability != 0)
            .map(|item| {
                let name = ident(item.name.to_pascal_case());
                let max_durability = item.max_durability;

                quote! {
                    Self::#name => #max_durability,
                }
            })
            .collect::<TokenStream>();

        let item_kind_to_enchantability_arms = items
            .iter()
            .filter(|item| item.enchantability != 0)
            .map(|item| {
                let name = ident(item.name.to_pascal_case());
                let ench = item.enchantability;

                quote! {
                    Self::#name => #ench,
                }
            })
            .collect::<TokenStream>();

        let item_kind_to_fireproof_arms = items
            .iter()
            .filter(|item| item.fireproof)
            .map(|item| {
                let name = ident(item.name.to_pascal_case());

                quote! {
                    Self::#name => true,
                }
            })
            .collect::<TokenStream>();

        Ok(quote! {
            #[doc = "Represents an item from the game"]
            #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
            #[repr(u16)]
            pub enum ItemKind {
                #[default]
                #(#item_kind_variants,)*
            }

            #[doc = "Contains food information about an item."]
            #[doc = ""]
            #[doc = "Only food items have a food component."]
            #[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
            pub struct FoodComponent {
                pub hunger: u16,
            pub saturation: f32,
            pub always_edible: bool,
            pub meat: bool,
            pub snack: bool,
        }

        impl ItemKind {
            #[doc = "Constructs a item kind from a raw item ID."]
            #[doc = ""]
            #[doc = "If the given ID is invalid, `None` is returned."]
            pub const fn from_raw(id: &str) -> Option<Self> {
                match id {
                    #item_kind_from_raw_id_arms
                    _ => None
                }
            }

            #[doc = "Gets the raw item ID from the item kind"]
            pub const fn to_raw(self) -> &str {
                match self {
                    #item_kind_to_raw_id_arms
                }
            }

            #[doc = "Construct an item kind for its snake_case name."]
            #[doc = ""]
            #[doc = "Returns `None` if the name is invalid."]
            #[allow(clippy::should_implement_trait)]
            pub fn from_str(name: &str) -> Option<ItemKind> {
                match name {
                    #item_kind_from_str_arms
                    _ => None
                }
            }

            #[doc = "Gets the snake_case name of this item kind."]
            pub const fn to_str(self) -> &'static str {
                match self {
                    #item_kind_to_str_arms
                }
            }

            #[doc = "Gets the translation key of this item kind."]
            pub const fn translation_key(self) -> &'static str {
                match self {
                    #item_kind_to_translation_key_arms
                }
            }

            #[doc = "Returns the maximum stack count."]
            pub const fn max_stack(self) -> i8 {
                match self {
                    #item_kind_to_max_stack_arms
                }
            }

            #[doc = "Returns a food component which stores hunger, saturation etc."]
            #[doc = ""]
            #[doc = "If the item kind can't be eaten, `None` will be returned."]
            pub const fn food_component(self) -> Option<FoodComponent> {
                match self {
                    #item_kind_to_food_component_arms
                    _ => None
                }
            }

            #[doc = "Returns the maximum durability before the item will break."]
            #[doc = ""]
            #[doc = "If the item doesn't have durability, `0` is returned."]
            pub const fn max_durability(self) -> u16 {
                match self {
                    #item_kind_to_max_durability_arms
                    _ => 0,
                }
            }

            #[doc = "Returns the enchantability of the item kind."]
            #[doc = ""]
            #[doc = "If the item doesn't have durability, `0` is returned."]
            pub const fn enchantability(self) -> u8 {
                match self {
                    #item_kind_to_enchantability_arms
                    _ => 0,
                }
            }

            #[doc = "Returns if the item can survive in fire/lava."]
            pub const fn fireproof(self) -> bool {
                #[allow(clippy::match_like_matches_macro)]
                match self {
                    #item_kind_to_fireproof_arms
                    _ => false
                }
            }

            /*
            #[doc = "Constructs an item kind from a block kind."]
            #[doc = ""]
            #[doc = "[`ItemKind::Air`] is used to indicate the absence of an item."]
            pub const fn from_block_kind(kind: BlockKind) -> Self {
                kind.to_item_kind()
            }

            #[doc = "Constructs a block kind from an item kind."]
            #[doc = ""]
            #[doc = "If the given item kind doesn't have a corresponding block kind, `None` is returned."]
            pub const fn to_block_kind(self) -> Option<BlockKind> {
                BlockKind::from_item_kind(self)
            }*/

            #[doc = "An array of all item kinds."]
            pub const ALL: [Self; #item_kind_count] = [#(Self::#item_kind_variants,)*];
        }
    })
}
