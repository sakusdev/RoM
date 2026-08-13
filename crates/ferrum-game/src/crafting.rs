//! Version-neutral crafting recipe matching.
use serde::{Deserialize,Serialize};
use thiserror::Error;
use crate::{ItemStack,validate_resource_location};

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
pub struct Ingredient(pub Vec<String>);
impl Ingredient{
 pub fn new(items:Vec<String>)->Result<Self,CraftingError>{if items.is_empty(){return Err(CraftingError::Empty);}for v in &items{if !validate_resource_location(v){return Err(CraftingError::InvalidId(v.clone()));}}Ok(Self(items))}
 #[must_use]pub fn matches(&self,s:&ItemStack)->bool{self.0.iter().any(|v|v==s.item())}
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
pub struct CraftingGrid{pub width:u8,pub height:u8,pub slots:Vec<Option<ItemStack>>}
impl CraftingGrid{
 pub fn new(width:u8,height:u8)->Result<Self,CraftingError>{let n=usize::from(width)*usize::from(height);if width==0||height==0||n>9{return Err(CraftingError::Grid);}Ok(Self{width,height,slots:vec![None;n]})}
 pub fn set(&mut self,index:usize,stack:Option<ItemStack>)->Result<(),CraftingError>{let slot=self.slots.get_mut(index).ok_or(CraftingError::Slot)?;*slot=stack;Ok(())}
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
pub struct ShapelessRecipe{pub id:String,pub ingredients:Vec<Ingredient>,pub result:ItemStack}
impl ShapelessRecipe{
 pub fn new(id:String,ingredients:Vec<Ingredient>,result:ItemStack)->Result<Self,CraftingError>{if !validate_resource_location(&id){return Err(CraftingError::InvalidId(id));}if ingredients.is_empty()||ingredients.len()>9{return Err(CraftingError::Empty);}Ok(Self{id,ingredients,result})}
 #[must_use]pub fn matches(&self,g:&CraftingGrid)->bool{let stacks=g.slots.iter().filter_map(Option::as_ref).collect::<Vec<_>>();if stacks.len()!=self.ingredients.len(){return false;}let mut used=vec![false;stacks.len()];for ing in &self.ingredients{let Some(i)=stacks.iter().enumerate().position(|(i,s)|!used[i]&&ing.matches(s))else{return false;};used[i]=true;}true}
}
#[derive(Debug,Error,PartialEq,Eq)]pub enum CraftingError{#[error("invalid id {0}")]InvalidId(String),#[error("empty recipe or ingredient")]Empty,#[error("invalid grid")]Grid,#[error("slot out of range")]Slot}
#[cfg(test)]mod tests{use super::*;#[test]fn shapeless(){let r=ShapelessRecipe::new("rom:test".into(),vec![Ingredient::new(vec!["minecraft:stone".into()]).unwrap()],ItemStack::new("minecraft:dirt",1).unwrap()).unwrap();let mut g=CraftingGrid::new(2,2).unwrap();g.set(2,Some(ItemStack::new("minecraft:stone",1).unwrap())).unwrap();assert!(r.matches(&g));}}
