#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MjpegAdmissionPolicy {
    PendingSlot,
    BoundedPrefetch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MjpegAdmissionStage {
    Capture,
    Decode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MjpegDropReason {
    PendingCapture,
    PendingDecode,
    BackpressureCapture,
    BackpressureDecode,
    WorkersBusy,
}

impl MjpegAdmissionPolicy {
    pub(super) fn from_prefetch_setting(setting: Option<&str>) -> Self {
        if setting == Some("0") {
            Self::PendingSlot
        } else {
            Self::BoundedPrefetch
        }
    }

    pub(super) fn check_pending(self, output_backpressured: bool) -> bool {
        self == Self::PendingSlot || output_backpressured
    }

    pub(super) fn drop_reason(
        self,
        pending: bool,
        output_backpressured: bool,
        stage: MjpegAdmissionStage,
    ) -> Option<MjpegDropReason> {
        if !pending || !self.check_pending(output_backpressured) {
            return None;
        }
        Some(match (output_backpressured, stage) {
            (false, MjpegAdmissionStage::Capture) => MjpegDropReason::PendingCapture,
            (false, MjpegAdmissionStage::Decode) => MjpegDropReason::PendingDecode,
            (true, MjpegAdmissionStage::Capture) => MjpegDropReason::BackpressureCapture,
            (true, MjpegAdmissionStage::Decode) => MjpegDropReason::BackpressureDecode,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_prefetch_is_default_and_zero_restores_pending_slot_policy() {
        for setting in [None, Some("1"), Some(""), Some("invalid")] {
            assert_eq!(
                MjpegAdmissionPolicy::from_prefetch_setting(setting),
                MjpegAdmissionPolicy::BoundedPrefetch
            );
        }
        assert_eq!(
            MjpegAdmissionPolicy::from_prefetch_setting(Some("0")),
            MjpegAdmissionPolicy::PendingSlot
        );
    }

    #[test]
    fn prefetch_keeps_decoding_while_output_has_capacity() {
        for stage in [MjpegAdmissionStage::Capture, MjpegAdmissionStage::Decode] {
            for pending in [false, true] {
                assert_eq!(
                    MjpegAdmissionPolicy::BoundedPrefetch.drop_reason(pending, false, stage),
                    None
                );
            }
        }
        assert!(!MjpegAdmissionPolicy::BoundedPrefetch.check_pending(false));
    }

    #[test]
    fn output_backpressure_preserves_one_pending_frame() {
        for policy in [
            MjpegAdmissionPolicy::BoundedPrefetch,
            MjpegAdmissionPolicy::PendingSlot,
        ] {
            for (stage, reason) in [
                (
                    MjpegAdmissionStage::Capture,
                    MjpegDropReason::BackpressureCapture,
                ),
                (
                    MjpegAdmissionStage::Decode,
                    MjpegDropReason::BackpressureDecode,
                ),
            ] {
                assert_eq!(policy.drop_reason(false, true, stage), None);
                assert_eq!(policy.drop_reason(true, true, stage), Some(reason));
            }
        }
    }

    #[test]
    fn legacy_policy_drops_when_pending_even_without_output_backpressure() {
        for (stage, reason) in [
            (
                MjpegAdmissionStage::Capture,
                MjpegDropReason::PendingCapture,
            ),
            (MjpegAdmissionStage::Decode, MjpegDropReason::PendingDecode),
        ] {
            assert_eq!(
                MjpegAdmissionPolicy::PendingSlot.drop_reason(false, false, stage),
                None
            );
            assert_eq!(
                MjpegAdmissionPolicy::PendingSlot.drop_reason(true, false, stage),
                Some(reason)
            );
        }
    }
}
