use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    HOTBAR_END, HOTBAR_START, Inventory, InventoryError, ItemStack, MAIN_INVENTORY_END,
    MAIN_INVENTORY_START, OFFHAND_SLOT, PLAYER_INVENTORY_SLOTS,
};

pub const PLAYER_CONTAINER_ID: i32 = 0;
pub const OUTSIDE_SLOT: i16 = -999;
pub const MAX_CONTAINER_SLOTS: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerClickKind {
    Pickup,
    QuickMove,
    Swap,
    Clone,
    Throw,
    QuickCraft,
    PickupAll,
}

impl ContainerClickKind {
    pub fn from_protocol_mode(mode: i32) -> Result<Self, ContainerError> {
        match mode {
            0 => Ok(Self::Pickup),
            1 => Ok(Self::QuickMove),
            2 => Ok(Self::Swap),
            3 => Ok(Self::Clone),
            4 => Ok(Self::Throw),
            5 => Ok(Self::QuickCraft),
            6 => Ok(Self::PickupAll),
            _ => Err(ContainerError::UnknownClickMode { mode }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerClick {
    pub container_id: i32,
    pub state_id: i32,
    pub slot: i16,
    pub button: i8,
    pub kind: ContainerClickKind,
    #[serde(default)]
    pub client_changed_slots: Vec<(i16, Option<ItemStack>)>,
    pub client_carried: Option<ItemStack>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerSnapshot {
    pub container_id: i32,
    pub state_id: i32,
    pub slots: Vec<Option<ItemStack>>,
    pub carried: Option<ItemStack>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenContainer {
    pub container_id: i32,
    pub slots: Vec<Option<ItemStack>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerMutation {
    pub accepted: bool,
    pub reason: Option<String>,
    pub snapshot: ContainerSnapshot,
    pub changed_player_slots: Vec<usize>,
    pub dropped: Vec<ItemStack>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotRef {
    Player(usize),
    External(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuickCraftState {
    drag_button: u8,
    slots: Vec<i16>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct InventorySession {
    state_id: i32,
    carried: Option<ItemStack>,
    open: Option<OpenContainer>,
    quick_craft: Option<QuickCraftState>,
}

impl InventorySession {
    #[must_use]
    pub const fn state_id(&self) -> i32 {
        self.state_id
    }

    #[must_use]
    pub fn carried(&self) -> Option<&ItemStack> {
        self.carried.as_ref()
    }

    #[must_use]
    pub fn current_container_id(&self) -> i32 {
        self.open
            .as_ref()
            .map_or(PLAYER_CONTAINER_ID, |open| open.container_id)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn open_container(
        &mut self,
        container_id: i32,
        slots: Vec<Option<ItemStack>>,
        inventory: &Inventory,
    ) -> Result<ContainerSnapshot, ContainerError> {
        if container_id <= PLAYER_CONTAINER_ID {
            return Err(ContainerError::InvalidContainerId { container_id });
        }
        if slots.len() > MAX_CONTAINER_SLOTS {
            return Err(ContainerError::TooManySlots {
                actual: slots.len(),
            });
        }
        self.open = Some(OpenContainer {
            container_id,
            slots,
        });
        self.quick_craft = None;
        self.bump_state();
        Ok(self.snapshot(inventory))
    }

    pub fn close_container(&mut self, inventory: &mut Inventory) -> ContainerMutation {
        let before = inventory.slots().to_vec();
        let mut dropped = Vec::new();
        if let Some(carried) = self.carried.take() {
            if let Some(remainder) = inventory.insert(carried) {
                dropped.push(remainder);
            }
        }
        self.open = None;
        self.quick_craft = None;
        self.bump_state();
        let changed_player_slots = changed_slots(&before, inventory.slots());
        ContainerMutation {
            accepted: true,
            reason: None,
            snapshot: self.snapshot(inventory),
            changed_player_slots,
            dropped,
        }
    }

    #[must_use]
    pub fn snapshot(&self, inventory: &Inventory) -> ContainerSnapshot {
        let container_id = self.current_container_id();
        let slots = if let Some(open) = &self.open {
            let mut combined = open.slots.clone();
            combined.extend_from_slice(&inventory.slots()[MAIN_INVENTORY_START..]);
            combined
        } else {
            inventory.slots().to_vec()
        };
        ContainerSnapshot {
            container_id,
            state_id: self.state_id,
            slots,
            carried: self.carried.clone(),
        }
    }

    pub fn click(
        &mut self,
        inventory: &mut Inventory,
        click: ContainerClick,
        creative: bool,
    ) -> Result<ContainerMutation, ContainerError> {
        let before = inventory.slots().to_vec();
        let expected_container = self.current_container_id();
        if click.container_id != expected_container {
            return Ok(self.rejected(
                inventory,
                format!(
                    "container {} is not open; expected {expected_container}",
                    click.container_id
                ),
            ));
        }
        if click.state_id != self.state_id {
            return Ok(self.rejected(
                inventory,
                format!(
                    "stale container state {}; expected {}",
                    click.state_id, self.state_id
                ),
            ));
        }
        if click.client_changed_slots.len() > MAX_CONTAINER_SLOTS {
            return Ok(self.rejected(inventory, "too many client changed slots".to_owned()));
        }

        let mut dropped = Vec::new();
        let result = match click.kind {
            ContainerClickKind::Pickup => {
                self.pickup(inventory, click.slot, click.button, &mut dropped)
            }
            ContainerClickKind::QuickMove => self.quick_move(inventory, click.slot),
            ContainerClickKind::Swap => self.swap(inventory, click.slot, click.button),
            ContainerClickKind::Clone => self.clone_stack(inventory, click.slot, creative),
            ContainerClickKind::Throw => {
                self.throw(inventory, click.slot, click.button, &mut dropped)
            }
            ContainerClickKind::QuickCraft => {
                self.quick_craft(inventory, click.slot, click.button, creative)
            }
            ContainerClickKind::PickupAll => self.pickup_all(inventory),
        };
        if let Err(error) = result {
            return Ok(self.rejected(inventory, error.to_string()));
        }

        self.bump_state();
        let changed_player_slots = changed_slots(&before, inventory.slots());
        Ok(ContainerMutation {
            accepted: true,
            reason: None,
            snapshot: self.snapshot(inventory),
            changed_player_slots,
            dropped,
        })
    }

    pub fn set_creative_slot(
        &mut self,
        inventory: &mut Inventory,
        slot: i16,
        stack: Option<ItemStack>,
        creative: bool,
    ) -> Result<ContainerMutation, ContainerError> {
        if !creative {
            return Ok(self.rejected(
                inventory,
                "creative slot update requires creative mode".to_owned(),
            ));
        }
        let before = inventory.slots().to_vec();
        let index = usize::try_from(slot).map_err(|_| ContainerError::SlotOutOfRange { slot })?;
        if index >= PLAYER_INVENTORY_SLOTS {
            return Ok(self.rejected(inventory, format!("inventory slot {slot} is out of range")));
        }
        inventory.set_slot(index, stack)?;
        self.bump_state();
        Ok(ContainerMutation {
            accepted: true,
            reason: None,
            snapshot: self.snapshot(inventory),
            changed_player_slots: changed_slots(&before, inventory.slots()),
            dropped: Vec::new(),
        })
    }

    fn rejected(&self, inventory: &Inventory, reason: String) -> ContainerMutation {
        ContainerMutation {
            accepted: false,
            reason: Some(reason),
            snapshot: self.snapshot(inventory),
            changed_player_slots: Vec::new(),
            dropped: Vec::new(),
        }
    }

    fn pickup(
        &mut self,
        inventory: &mut Inventory,
        slot: i16,
        button: i8,
        dropped: &mut Vec<ItemStack>,
    ) -> Result<(), ContainerError> {
        if button != 0 && button != 1 {
            return Err(ContainerError::InvalidButton { button });
        }
        if slot == OUTSIDE_SLOT {
            if let Some(mut carried) = self.carried.take() {
                if button == 0 || carried.count() == 1 {
                    dropped.push(carried);
                } else {
                    dropped.push(carried.copy_with_count(1)?);
                    carried = carried.copy_with_count(carried.count() - 1)?;
                    self.carried = Some(carried);
                }
            }
            return Ok(());
        }
        let target = self.resolve_slot(slot)?;
        let slot_stack = self.get_slot(inventory, target)?.cloned();
        match (self.carried.take(), slot_stack) {
            (None, None) => {}
            (None, Some(stack)) => {
                if button == 0 || stack.count() == 1 {
                    self.set_slot(inventory, target, None)?;
                    self.carried = Some(stack);
                } else {
                    let take = stack.count().div_ceil(2);
                    self.carried = Some(stack.copy_with_count(take)?);
                    self.set_slot(
                        inventory,
                        target,
                        Some(stack.copy_with_count(stack.count() - take)?),
                    )?;
                }
            }
            (Some(carried), None) => {
                if button == 0 || carried.count() == 1 {
                    self.set_slot(inventory, target, Some(carried))?;
                } else {
                    self.set_slot(inventory, target, Some(carried.copy_with_count(1)?))?;
                    self.carried = Some(carried.copy_with_count(carried.count() - 1)?);
                }
            }
            (Some(carried), Some(existing)) => {
                if existing.can_merge(&carried) && existing.count() < existing.max_count() {
                    let amount = if button == 0 {
                        carried.count().min(existing.remaining_capacity())
                    } else {
                        1.min(existing.remaining_capacity())
                    };
                    self.set_slot(
                        inventory,
                        target,
                        Some(existing.copy_with_count(existing.count() + amount)?),
                    )?;
                    if amount < carried.count() {
                        self.carried = Some(carried.copy_with_count(carried.count() - amount)?);
                    }
                } else {
                    self.set_slot(inventory, target, Some(carried))?;
                    self.carried = Some(existing);
                }
            }
        }
        Ok(())
    }

    fn quick_move(&mut self, inventory: &mut Inventory, slot: i16) -> Result<(), ContainerError> {
        let source = self.resolve_slot(slot)?;
        let Some(stack) = self.get_slot(inventory, source)?.cloned() else {
            return Ok(());
        };
        let remainder = match source {
            SlotRef::External(_) => inventory.insert(stack),
            SlotRef::Player(_index) if self.open.is_some() => self.insert_external(stack)?,
            SlotRef::Player(index)
                if (MAIN_INVENTORY_START..=MAIN_INVENTORY_END).contains(&index) =>
            {
                insert_player_range(inventory, stack, HOTBAR_START, HOTBAR_END)?
            }
            SlotRef::Player(index) if (HOTBAR_START..=HOTBAR_END).contains(&index) => {
                insert_player_range(inventory, stack, MAIN_INVENTORY_START, MAIN_INVENTORY_END)?
            }
            SlotRef::Player(_) => inventory.insert(stack),
        };
        self.set_slot(inventory, source, remainder)?;
        Ok(())
    }

    fn swap(
        &mut self,
        inventory: &mut Inventory,
        slot: i16,
        button: i8,
    ) -> Result<(), ContainerError> {
        let source = self.resolve_slot(slot)?;
        let player_index = match button {
            0..=8 => HOTBAR_START + usize::try_from(button).expect("non-negative button"),
            40 => OFFHAND_SLOT,
            _ => return Err(ContainerError::InvalidButton { button }),
        };
        if source == SlotRef::Player(player_index) {
            return Ok(());
        }
        let source_stack = self.get_slot(inventory, source)?.cloned();
        let target_stack = inventory.slot(player_index)?.cloned();
        self.set_slot(inventory, source, target_stack)?;
        inventory.set_slot(player_index, source_stack)?;
        Ok(())
    }

    fn clone_stack(
        &mut self,
        inventory: &Inventory,
        slot: i16,
        creative: bool,
    ) -> Result<(), ContainerError> {
        if !creative {
            return Err(ContainerError::CreativeRequired);
        }
        let target = self.resolve_slot(slot)?;
        self.carried = self
            .get_slot(inventory, target)?
            .map(|stack| stack.copy_with_count(stack.max_count()))
            .transpose()?;
        Ok(())
    }

    fn throw(
        &mut self,
        inventory: &mut Inventory,
        slot: i16,
        button: i8,
        dropped: &mut Vec<ItemStack>,
    ) -> Result<(), ContainerError> {
        if button != 0 && button != 1 {
            return Err(ContainerError::InvalidButton { button });
        }
        let target = self.resolve_slot(slot)?;
        let Some(stack) = self.get_slot(inventory, target)?.cloned() else {
            return Ok(());
        };
        if button == 1 || stack.count() == 1 {
            self.set_slot(inventory, target, None)?;
            dropped.push(stack);
        } else {
            dropped.push(stack.copy_with_count(1)?);
            self.set_slot(
                inventory,
                target,
                Some(stack.copy_with_count(stack.count() - 1)?),
            )?;
        }
        Ok(())
    }

    fn quick_craft(
        &mut self,
        inventory: &mut Inventory,
        slot: i16,
        button: i8,
        creative: bool,
    ) -> Result<(), ContainerError> {
        let encoded = u8::try_from(button).map_err(|_| ContainerError::InvalidButton { button })?;
        let stage = encoded & 3;
        let drag_button = encoded >> 2;
        match stage {
            0 => {
                if self.carried.is_none() || drag_button > 2 || (drag_button == 2 && !creative) {
                    self.quick_craft = None;
                    return Err(ContainerError::InvalidQuickCraft);
                }
                self.quick_craft = Some(QuickCraftState {
                    drag_button,
                    slots: Vec::new(),
                });
            }
            1 => {
                self.resolve_slot(slot)?;
                let state = self
                    .quick_craft
                    .as_mut()
                    .ok_or(ContainerError::InvalidQuickCraft)?;
                if !state.slots.contains(&slot) {
                    state.slots.push(slot);
                }
            }
            2 => {
                let state = self
                    .quick_craft
                    .take()
                    .ok_or(ContainerError::InvalidQuickCraft)?;
                self.finish_quick_craft(inventory, state, creative)?;
            }
            _ => return Err(ContainerError::InvalidQuickCraft),
        }
        Ok(())
    }

    fn finish_quick_craft(
        &mut self,
        inventory: &mut Inventory,
        state: QuickCraftState,
        creative: bool,
    ) -> Result<(), ContainerError> {
        let Some(carried) = self.carried.clone() else {
            return Ok(());
        };
        if state.slots.is_empty() {
            return Ok(());
        }
        if state.drag_button == 2 {
            if !creative {
                return Err(ContainerError::CreativeRequired);
            }
            for slot in state.slots {
                let target = self.resolve_slot(slot)?;
                self.set_slot(
                    inventory,
                    target,
                    Some(carried.copy_with_count(carried.max_count())?),
                )?;
            }
            return Ok(());
        }
        let per_slot = if state.drag_button == 1 {
            1
        } else {
            (carried.count() / u32::try_from(state.slots.len()).expect("slot count fits u32"))
                .max(1)
        };
        let mut remaining = carried.count();
        for slot in state.slots {
            if remaining == 0 {
                break;
            }
            let target = self.resolve_slot(slot)?;
            let existing = self.get_slot(inventory, target)?.cloned();
            let capacity = existing
                .as_ref()
                .filter(|existing| existing.can_merge(&carried))
                .map_or_else(
                    || {
                        if existing.is_none() {
                            carried.max_count()
                        } else {
                            0
                        }
                    },
                    ItemStack::remaining_capacity,
                );
            let moved = per_slot.min(capacity).min(remaining);
            if moved == 0 {
                continue;
            }
            let next = existing
                .map(|existing| existing.copy_with_count(existing.count() + moved))
                .unwrap_or_else(|| carried.copy_with_count(moved))?;
            self.set_slot(inventory, target, Some(next))?;
            remaining -= moved;
        }
        self.carried = if remaining == 0 {
            None
        } else {
            Some(carried.copy_with_count(remaining)?)
        };
        Ok(())
    }

    fn pickup_all(&mut self, inventory: &mut Inventory) -> Result<(), ContainerError> {
        let Some(mut carried) = self.carried.take() else {
            return Ok(());
        };
        let total_slots = self.snapshot(inventory).slots.len();
        for raw in 0..total_slots {
            if carried.count() >= carried.max_count() {
                break;
            }
            let slot = i16::try_from(raw)
                .map_err(|_| ContainerError::SlotOutOfRange { slot: i16::MAX })?;
            let target = self.resolve_slot(slot)?;
            let Some(existing) = self.get_slot(inventory, target)?.cloned() else {
                continue;
            };
            if !existing.can_merge(&carried) {
                continue;
            }
            let moved = (carried.max_count() - carried.count()).min(existing.count());
            carried = carried.copy_with_count(carried.count() + moved)?;
            if moved == existing.count() {
                self.set_slot(inventory, target, None)?;
            } else {
                self.set_slot(
                    inventory,
                    target,
                    Some(existing.copy_with_count(existing.count() - moved)?),
                )?;
            }
        }
        self.carried = Some(carried);
        Ok(())
    }

    fn resolve_slot(&self, slot: i16) -> Result<SlotRef, ContainerError> {
        let index = usize::try_from(slot).map_err(|_| ContainerError::SlotOutOfRange { slot })?;
        if let Some(open) = &self.open {
            if index < open.slots.len() {
                return Ok(SlotRef::External(index));
            }
            let player_index = MAIN_INVENTORY_START + index - open.slots.len();
            if player_index < PLAYER_INVENTORY_SLOTS {
                return Ok(SlotRef::Player(player_index));
            }
        } else if index < PLAYER_INVENTORY_SLOTS {
            return Ok(SlotRef::Player(index));
        }
        Err(ContainerError::SlotOutOfRange { slot })
    }

    fn get_slot<'a>(
        &'a self,
        inventory: &'a Inventory,
        slot: SlotRef,
    ) -> Result<Option<&'a ItemStack>, ContainerError> {
        match slot {
            SlotRef::Player(index) => Ok(inventory.slot(index)?),
            SlotRef::External(index) => Ok(self
                .open
                .as_ref()
                .and_then(|open| open.slots.get(index))
                .and_then(Option::as_ref)),
        }
    }

    fn set_slot(
        &mut self,
        inventory: &mut Inventory,
        slot: SlotRef,
        stack: Option<ItemStack>,
    ) -> Result<(), ContainerError> {
        match slot {
            SlotRef::Player(index) => {
                inventory.set_slot(index, stack)?;
            }
            SlotRef::External(index) => {
                let open = self.open.as_mut().ok_or(ContainerError::NoOpenContainer)?;
                let target = open
                    .slots
                    .get_mut(index)
                    .ok_or(ContainerError::SlotOutOfRange {
                        slot: i16::try_from(index).unwrap_or(i16::MAX),
                    })?;
                *target = stack;
            }
        }
        Ok(())
    }

    fn insert_external(
        &mut self,
        mut stack: ItemStack,
    ) -> Result<Option<ItemStack>, ContainerError> {
        let open = self.open.as_mut().ok_or(ContainerError::NoOpenContainer)?;
        for existing in open.slots.iter_mut().flatten() {
            if existing.can_merge(&stack) && existing.remaining_capacity() > 0 {
                let moved = existing.remaining_capacity().min(stack.count());
                *existing = existing.copy_with_count(existing.count() + moved)?;
                if moved == stack.count() {
                    return Ok(None);
                }
                stack = stack.copy_with_count(stack.count() - moved)?;
            }
        }
        for target in &mut open.slots {
            if target.is_none() {
                *target = Some(stack);
                return Ok(None);
            }
        }
        Ok(Some(stack))
    }

    fn bump_state(&mut self) {
        self.state_id = self.state_id.wrapping_add(1) & i32::MAX;
    }
}

fn insert_player_range(
    inventory: &mut Inventory,
    mut stack: ItemStack,
    start: usize,
    end: usize,
) -> Result<Option<ItemStack>, ContainerError> {
    for index in start..=end {
        let Some(existing) = inventory.slot(index)?.cloned() else {
            continue;
        };
        if existing.can_merge(&stack) && existing.remaining_capacity() > 0 {
            let moved = existing.remaining_capacity().min(stack.count());
            inventory.set_slot(
                index,
                Some(existing.copy_with_count(existing.count() + moved)?),
            )?;
            if moved == stack.count() {
                return Ok(None);
            }
            stack = stack.copy_with_count(stack.count() - moved)?;
        }
    }
    for index in start..=end {
        if inventory.slot(index)?.is_none() {
            inventory.set_slot(index, Some(stack))?;
            return Ok(None);
        }
    }
    Ok(Some(stack))
}

fn changed_slots(before: &[Option<ItemStack>], after: &[Option<ItemStack>]) -> Vec<usize> {
    before
        .iter()
        .zip(after)
        .enumerate()
        .filter(|(_, (before, after))| before != after)
        .map(|(index, _)| index)
        .collect()
}

#[derive(Debug, Error)]
pub enum ContainerError {
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error("unknown container click mode {mode}")]
    UnknownClickMode { mode: i32 },
    #[error("invalid container ID {container_id}")]
    InvalidContainerId { container_id: i32 },
    #[error("container has {actual} slots; limit is {MAX_CONTAINER_SLOTS}")]
    TooManySlots { actual: usize },
    #[error("container slot {slot} is out of range")]
    SlotOutOfRange { slot: i16 },
    #[error("invalid inventory button {button}")]
    InvalidButton { button: i8 },
    #[error("creative inventory operation requires creative mode")]
    CreativeRequired,
    #[error("invalid quick-craft sequence")]
    InvalidQuickCraft,
    #[error("no external container is open")]
    NoOpenContainer,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stone(count: u32) -> ItemStack {
        ItemStack::new("minecraft:stone", count).unwrap()
    }

    #[test]
    fn rejects_stale_state_with_authoritative_snapshot() {
        let mut inventory = Inventory::new();
        inventory.set_slot(9, Some(stone(4))).unwrap();
        let mut session = InventorySession::default();
        let mutation = session
            .click(
                &mut inventory,
                ContainerClick {
                    container_id: 0,
                    state_id: 4,
                    slot: 9,
                    button: 0,
                    kind: ContainerClickKind::Pickup,
                    client_changed_slots: Vec::new(),
                    client_carried: None,
                },
                false,
            )
            .unwrap();
        assert!(!mutation.accepted);
        assert_eq!(mutation.snapshot.slots[9].as_ref().unwrap().count(), 4);
    }

    #[test]
    fn pickup_and_quick_move_are_authoritative() {
        let mut inventory = Inventory::new();
        inventory.set_slot(9, Some(stone(8))).unwrap();
        let mut session = InventorySession::default();
        let first = session
            .click(
                &mut inventory,
                ContainerClick {
                    container_id: 0,
                    state_id: 0,
                    slot: 9,
                    button: 1,
                    kind: ContainerClickKind::Pickup,
                    client_changed_slots: Vec::new(),
                    client_carried: None,
                },
                false,
            )
            .unwrap();
        assert!(first.accepted);
        assert_eq!(first.snapshot.carried.as_ref().unwrap().count(), 4);
        assert_eq!(inventory.slot(9).unwrap().unwrap().count(), 4);
    }

    #[test]
    fn external_container_combines_player_storage() {
        let inventory = Inventory::new();
        let mut session = InventorySession::default();
        let snapshot = session
            .open_container(1, vec![Some(stone(2)); 27], &inventory)
            .unwrap();
        assert_eq!(snapshot.container_id, 1);
        assert_eq!(
            snapshot.slots.len(),
            27 + PLAYER_INVENTORY_SLOTS - MAIN_INVENTORY_START
        );
    }
}
