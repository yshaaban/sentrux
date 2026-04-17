#[path = "agent_guidance_finding.rs"]
mod finding;
#[path = "agent_guidance_obligation.rs"]
mod obligation;
#[path = "agent_guidance_packets.rs"]
mod packets;
#[path = "agent_guidance_shared.rs"]
mod shared;

pub(crate) use finding::{fix_hint_for_value, repair_packet_for_finding};
pub(crate) use obligation::{
    obligation_confidence, obligation_evidence, obligation_files, obligation_fix_hint,
    obligation_line, obligation_message, obligation_origin, obligation_score_0_10000,
    obligation_severity, obligation_trust_tier, repair_packet_for_obligation,
};
pub(crate) use packets::RepairPacket;
#[cfg(test)]
pub(crate) use packets::RepairPacketRequiredFields;

#[cfg(test)]
#[path = "agent_guidance_tests.rs"]
mod tests;
