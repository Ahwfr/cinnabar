use super::{MAX_IMPORT_WRITES, State, cinnabar};
use anyhow::{Result, bail};

/// A use pulse staged by the running frame callback and published only when it succeeds.
#[derive(Default)]
pub(super) struct ItemUsePolicy {
    pending_pulse: bool,
    pending_placement: Option<crate::CreativePlacementOffer>,
    writes: u32,
    /// The current frame's committed pulse, read before the action filter runs.
    pub pulse: bool,
    pub placement: Option<crate::CreativePlacementOffer>,
}

impl ItemUsePolicy {
    /// Publishes a pulse only from a callback that saw a gameplay frame.
    pub fn commit(&mut self, gameplay: bool) {
        self.pulse = std::mem::take(&mut self.pending_pulse) && gameplay;
        self.placement = self.pending_placement.take().filter(|_| gameplay);
    }
}

impl cinnabar::extension::item_use::Host for State {
    fn offer_creative_placement(
        &mut self,
        position: (i32, i32, i32),
        name: String,
        states: String,
    ) -> Result<Result<(), String>> {
        self.item_use_policy.writes += 1;
        if self.item_use_policy.writes > MAX_IMPORT_WRITES {
            bail!("item-use import budget exhausted");
        }
        if !self.grants.item_use || !self.grants.inventory || self.snapshot.is_none() {
            return Ok(Err(
                "Creative placement requires gameplay, item use and inventory".into(),
            ));
        }
        let position = [position.0, position.1, position.2];
        if position.iter().any(|v| v.unsigned_abs() > 30_000_000)
            || !name.starts_with("minecraft:")
            || name.len() > 128
            || states.len() > 2048
        {
            return Ok(Err("invalid Creative placement offer".into()));
        }
        self.item_use_policy.pending_placement = Some(crate::CreativePlacementOffer {
            position,
            name,
            states,
        });
        Ok(Ok(()))
    }

    /// Stages one use press edge while gameplay owns input and the use control is held.
    fn pulse_use(&mut self) -> Result<Result<(), String>> {
        self.item_use_policy.writes += 1;
        if self.item_use_policy.writes > MAX_IMPORT_WRITES {
            bail!("item-use import budget exhausted");
        }
        if !self.grants.item_use {
            return Ok(Err("item-use capability denied".into()));
        }
        if self.snapshot.is_none() || !self.gameplay_facts.use_held {
            return Ok(Err("use pulse requires a held current gameplay use".into()));
        }
        self.item_use_policy.pending_pulse = true;
        Ok(Ok(()))
    }
}
