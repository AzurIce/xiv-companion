/// Resolved external Document trigger call, before Scheduler mapping lookup.
/// The Scheduler index is its ordinal in the live Document child list, not an
/// identity inferred from a Timeline. Slot numbers are internal, zero based.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxSchedulerTrigger {
    pub scheduler_index: usize,
    pub slot: i32,
}

impl VfxSchedulerTrigger {
    /// Document 0x1403b1de0 rejects negative arguments and absent live children,
    /// then forwards `trigger_number - 1` to 0x1403bc400. In particular, zero
    /// forwards slot -1; this models routing, not a safe mapping-table access.
    /// Callers must validate the slot against their resolved Scheduler table.
    pub fn from_document_input(
        live_scheduler_count: usize,
        scheduler_index: i32,
        trigger_number: i32,
    ) -> Option<Self> {
        let scheduler_index = usize::try_from(scheduler_index).ok()?;
        if trigger_number < 0 || scheduler_index >= live_scheduler_count {
            return None;
        }
        Some(Self {
            scheduler_index,
            slot: trigger_number - 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_trigger_uses_live_scheduler_ordinal_and_one_based_number() {
        assert_eq!(
            VfxSchedulerTrigger::from_document_input(2, 1, 10),
            Some(VfxSchedulerTrigger {
                scheduler_index: 1,
                slot: 9
            })
        );
        assert_eq!(
            VfxSchedulerTrigger::from_document_input(1, 0, 1)
                .unwrap()
                .slot,
            0
        );
        assert_eq!(
            VfxSchedulerTrigger::from_document_input(1, 0, 0)
                .unwrap()
                .slot,
            -1
        );
        assert_eq!(
            VfxSchedulerTrigger::from_document_input(1, 0, i32::MAX)
                .unwrap()
                .slot,
            i32::MAX - 1
        );
        for (count, index, number) in [
            (0, 0, 1),
            (1, 1, 1),
            (2, -1, 1),
            (2, 0, -1),
            (2, 0, i32::MIN),
        ] {
            assert_eq!(
                VfxSchedulerTrigger::from_document_input(count, index, number),
                None
            );
        }
    }
}
