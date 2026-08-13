//! Bounded spatial math used by movement, reach, and entity queries.
use serde::{Deserialize,Serialize};
use thiserror::Error;

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize)]pub struct Aabb{pub min:[f64;3],pub max:[f64;3]}
impl Aabb{
 pub fn new(min:[f64;3],max:[f64;3])->Result<Self,SpatialError>{if min.into_iter().chain(max).any(|v|!v.is_finite()){return Err(SpatialError::NonFinite);}if (0..3).any(|i|min[i]>max[i]){return Err(SpatialError::InvalidBounds);}Ok(Self{min,max})}
 #[must_use]pub fn translated(self,d:[f64;3])->Self{Self{min:[self.min[0]+d[0],self.min[1]+d[1],self.min[2]+d[2]],max:[self.max[0]+d[0],self.max[1]+d[1],self.max[2]+d[2]]}}
 #[must_use]pub fn intersects(self,other:Self)->bool{(0..3).all(|i|self.max[i]>other.min[i]&&self.min[i]<other.max[i])}
 #[must_use]pub fn contains(self,p:[f64;3])->bool{(0..3).all(|i|p[i]>=self.min[i]&&p[i]<=self.max[i])}
 #[must_use]pub fn expanded(self,a:f64)->Self{Self{min:[self.min[0]-a,self.min[1]-a,self.min[2]-a],max:[self.max[0]+a,self.max[1]+a,self.max[2]+a]}}
 #[must_use]pub fn center(self)->[f64;3]{[(self.min[0]+self.max[0])*0.5,(self.min[1]+self.max[1])*0.5,(self.min[2]+self.max[2])*0.5]}
}
#[must_use]pub fn distance_squared(a:[f64;3],b:[f64;3])->f64{let x=a[0]-b[0];let y=a[1]-b[1];let z=a[2]-b[2];x*x+y*y+z*z}
#[must_use]pub fn within_reach(a:[f64;3],b:[f64;3],reach:f64)->bool{reach.is_finite()&&reach>=0.0&&distance_squared(a,b)<=reach*reach}
#[must_use]pub fn normalize_horizontal(v:[f64;3])->[f64;3]{let l=(v[0]*v[0]+v[2]*v[2]).sqrt();if l<1e-12||!l.is_finite(){[0.0,0.0,0.0]}else{[v[0]/l,0.0,v[2]/l]}}
#[must_use]pub fn clamp_vector(v:[f64;3],max:f64)->[f64;3]{if !max.is_finite()||max<=0.0{return[0.0;3];}let l=(v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt();if l<=max||l==0.0{v}else{let s=max/l;[v[0]*s,v[1]*s,v[2]*s]}}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]pub struct ChunkPos{pub x:i32,pub z:i32}
impl ChunkPos{#[must_use]pub fn from_world(p:[f64;3])->Self{Self{x:(p[0].floor()as i32).div_euclid(16),z:(p[2].floor()as i32).div_euclid(16)}}#[must_use]pub fn distance(self,o:Self)->u32{self.x.abs_diff(o.x).max(self.z.abs_diff(o.z))}}
#[derive(Debug,Error,PartialEq,Eq)]pub enum SpatialError{#[error("spatial coordinate is not finite")]NonFinite,#[error("spatial bounds are inverted")]InvalidBounds}
#[cfg(test)]mod tests{use super::*;#[test]fn negative_chunks_floor_correctly(){assert_eq!(ChunkPos::from_world([-0.1,0.0,-16.1]),ChunkPos{x:-1,z:-2});}#[test]fn boxes_intersect(){let a=Aabb::new([0.0;3],[1.0;3]).unwrap();let b=Aabb::new([0.5;3],[2.0;3]).unwrap();assert!(a.intersects(b));}#[test]fn reach_is_squared_distance(){assert!(within_reach([0.0;3],[3.0,4.0,0.0],5.0));}}
