//! Proposed 83EX trial envelope, evaluated offline only. These are conservative
//! experiment cutoffs, not Lenovo limits, proof of cooling or write permission.
use legion_core::{
    FanTelemetry, FanValidation, TelemetryState, TelemetryUnit, TelemetryValue, ValueClassification,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ThermalTrialProtocol {
    source: &'static str,
    pre_cpu_max_c: f64,
    pre_gpu_max_c: f64,
    post_cpu_cancel_c: f64,
    post_gpu_cancel_c: f64,
    post_rise_cancel_c: f64,
    pre_channel_min_rpm: u32,
    post_channel_cancel_below_rpm: u32,
    maximum_sample_age_ms: u64,
    baseline_seconds: u64,
    observation_seconds: u64,
    proposed_sample_interval_ms: u64,
    authorizes_write: bool,
}

pub fn proposed_83ex_thermal_protocol() -> ThermalTrialProtocol {
    ThermalTrialProtocol {
        source: "proposed-experiment-policy; not-firmware-limits; no-host-acquisition",
        pre_cpu_max_c: 55.0,
        pre_gpu_max_c: 50.0,
        post_cpu_cancel_c: 70.0,
        post_gpu_cancel_c: 65.0,
        post_rise_cancel_c: 10.0,
        pre_channel_min_rpm: 1200,
        post_channel_cancel_below_rpm: 1000,
        maximum_sample_age_ms: 3000,
        baseline_seconds: 60,
        observation_seconds: 120,
        proposed_sample_interval_ms: 1000,
        authorizes_write: false,
    }
}

/// Sensor mapping must already have been established independently. Availability
/// and a public provenance string alone never establish physical sensor identity.
pub struct TrialSample<'a> {
    pub cpu: &'a TelemetryValue<f64>,
    pub gpu: &'a TelemetryValue<f64>,
    pub fans: [&'a FanTelemetry; 2],
    /// Ages at the observation instant, CPU/GPU/channel1/channel2 respectively.
    pub ages_ms: [Option<u64>; 4],
    pub independent_sensor_mapping_confirmed: bool,
    pub ac_online: Option<bool>,
    pub custom_profile: Option<bool>,
    pub fan_full_speed: Option<bool>,
    pub intensive_workload_absent: Option<bool>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrialDisposition {
    CandidateOnlyNotAuthorization,
    CancelBeforeWrite,
    WithinObservationEnvelopeNotAcceptance,
    AnomalyAfterAttemptNotRollback,
}

#[derive(Debug, Serialize)]
pub struct TrialAssessment {
    pub disposition: TrialDisposition,
    pub reasons: Vec<&'static str>,
    pub authorizes_write: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrialPhase {
    BeforeWrite,
    AfterAttempt,
}

/// The phase is explicit: missing post-attempt baseline must never be treated
/// as pre-write cancellation. Does not read,
/// monitor, sleep, retry, invoke a callback, interrupt SFAN or emit authorization.
pub fn assess_trial_sample(
    sample: &TrialSample<'_>,
    phase: TrialPhase,
    baseline: Option<&TrialSample<'_>>,
) -> TrialAssessment {
    let policy = proposed_83ex_thermal_protocol();
    let post = phase == TrialPhase::AfterAttempt;
    let mut reasons = Vec::new();
    if (post && baseline.is_none()) || (!post && baseline.is_some()) {
        reasons.push("missing post-attempt baseline or contradictory phase evidence");
    }
    if !sample.independent_sensor_mapping_confirmed {
        reasons.push("sensor mapping/provenance not independently confirmed");
    }
    if sample.ac_online != Some(true)
        || sample.custom_profile != Some(true)
        || sample.fan_full_speed != Some(false)
        || sample.intensive_workload_absent != Some(true)
    {
        reasons.push("AC/custom/FullSpeed/no-intensive-load condition missing or changed");
    }
    if sample
        .ages_ms
        .iter()
        .any(|age| age.is_none_or(|age| age > policy.maximum_sample_age_ms))
    {
        reasons.push("missing or stale observation");
    }
    let valid_temperature = |value: &TelemetryValue<f64>| {
        value.state == TelemetryState::Available
            && value.unit == TelemetryUnit::Celsius
            && value.classification == ValueClassification::InstantaneousMeasurement
            && value
                .value
                .is_some_and(|v| v.is_finite() && (5.0..=110.0).contains(&v))
    };
    if !valid_temperature(sample.cpu) || !valid_temperature(sample.gpu) {
        reasons.push(
            "missing/error/unvalidated temperature, wrong magnitude/unit or outside trial range",
        );
    } else {
        let cpu = sample.cpu.value.unwrap();
        let gpu = sample.gpu.value.unwrap();
        if if post {
            cpu >= policy.post_cpu_cancel_c || gpu >= policy.post_gpu_cancel_c
        } else {
            cpu > policy.pre_cpu_max_c || gpu > policy.pre_gpu_max_c
        } {
            reasons.push("temperature outside proposed trial envelope");
        }
        if let Some(before) = baseline {
            if assess_trial_sample(before, TrialPhase::BeforeWrite, None).disposition
                != TrialDisposition::CandidateOnlyNotAuthorization
            {
                reasons.push("pre-write baseline invalid");
            } else if cpu - before.cpu.value.unwrap() >= policy.post_rise_cancel_c
                || gpu - before.gpu.value.unwrap() >= policy.post_rise_cancel_c
            {
                reasons.push("temperature rise at least 10 C from pre-write baseline");
            }
        }
    }
    if sample.fans[0].fan == sample.fans[1].fan {
        reasons.push("duplicate fan channel identity");
    }
    for (index, fan) in sample.fans.iter().enumerate() {
        let reading = &fan.reported_speed;
        if fan.validation != FanValidation::Validated
            || reading.state != TelemetryState::Available
            || reading.unit != TelemetryUnit::Rpm
            || reading.classification != ValueClassification::InstantaneousMeasurement
            || reading.value.is_none_or(|rpm| rpm > 6000)
        {
            reasons.push("missing/error/unvalidated RPM channel; no stopped-fan inference");
            continue;
        }
        let rpm = reading.value.unwrap();
        if (!post && rpm < policy.pre_channel_min_rpm)
            || (post && rpm < policy.post_channel_cancel_below_rpm)
        {
            reasons.push("RPM below conservative experiment cutoff, not proof of a stopped fan");
        }
        if let Some(before) = baseline {
            if before.fans[index]
                .reported_speed
                .value
                .is_some_and(|previous| rpm.saturating_mul(2) < previous)
            {
                reasons.push("RPM fell by more than half; suspend experiment, do not rewrite");
            }
        }
    }
    TrialAssessment {
        disposition: match (post, reasons.is_empty()) {
            (false, true) => TrialDisposition::CandidateOnlyNotAuthorization,
            (false, false) => TrialDisposition::CancelBeforeWrite,
            (true, true) => TrialDisposition::WithinObservationEnvelopeNotAcceptance,
            (true, false) => TrialDisposition::AnomalyAfterAttemptNotRollback,
        },
        reasons,
        authorizes_write: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_core::{ConfidenceLevel, FanInterpretation, TelemetryArtifact, TelemetryProvenance};
    fn value<T>(value: T, unit: TelemetryUnit) -> TelemetryValue<T> {
        TelemetryValue {
            value: Some(value),
            unit,
            classification: ValueClassification::InstantaneousMeasurement,
            state: TelemetryState::Available,
            provenance: TelemetryProvenance {
                artifact: TelemetryArtifact::StaticSimulation,
                source: "in-memory-fixture".into(),
                confidence: ConfidenceLevel::Low,
            },
        }
    }
    fn fan(name: &str) -> FanTelemetry {
        FanTelemetry {
            fan: name.into(),
            reported_speed: value(1500, TelemetryUnit::Rpm),
            validation: FanValidation::Validated,
            interpretation: FanInterpretation::ReportedValue,
        }
    }
    fn sample<'a>(
        cpu: &'a TelemetryValue<f64>,
        gpu: &'a TelemetryValue<f64>,
        fans: [&'a FanTelemetry; 2],
    ) -> TrialSample<'a> {
        TrialSample {
            cpu,
            gpu,
            fans,
            ages_ms: [Some(0); 4],
            independent_sensor_mapping_confirmed: true,
            ac_online: Some(true),
            custom_profile: Some(true),
            fan_full_speed: Some(false),
            intensive_workload_absent: Some(true),
        }
    }
    #[test]
    fn accepts_only_the_envelope_never_authorizes_a_write() {
        let (cpu, gpu, a, b) = (
            value(55.0, TelemetryUnit::Celsius),
            value(50.0, TelemetryUnit::Celsius),
            fan("channel1"),
            fan("channel2"),
        );
        let before = sample(&cpu, &gpu, [&a, &b]);
        let result = assess_trial_sample(&before, TrialPhase::BeforeWrite, None);
        assert_eq!(
            result.disposition,
            TrialDisposition::CandidateOnlyNotAuthorization
        );
        assert!(!result.authorizes_write);
        assert_eq!(
            assess_trial_sample(&before, TrialPhase::AfterAttempt, Some(&before)).disposition,
            TrialDisposition::WithinObservationEnvelopeNotAcceptance
        );
    }
    #[test]
    fn missing_gpu_wrong_unit_limits_nan_and_unknown_sources_fail_closed() {
        let (cpu, mut gpu, a, b) = (
            value(43.5, TelemetryUnit::Celsius),
            value(40.0, TelemetryUnit::Celsius),
            fan("channel1"),
            fan("channel2"),
        );
        for scenario in 0..6 {
            gpu.value = Some(40.0);
            gpu.unit = TelemetryUnit::Celsius;
            gpu.classification = ValueClassification::InstantaneousMeasurement;
            gpu.state = TelemetryState::Available;
            match scenario {
                0 => gpu.value = None,
                1 => gpu.unit = TelemetryUnit::Watts,
                2 => gpu.classification = ValueClassification::ConfiguredLimit,
                3 => gpu.value = Some(f64::NAN),
                4 => gpu.state = TelemetryState::Unvalidated,
                _ => gpu.value = Some(50.1),
            }
            assert_eq!(
                assess_trial_sample(&sample(&cpu, &gpu, [&a, &b]), TrialPhase::BeforeWrite, None)
                    .disposition,
                TrialDisposition::CancelBeforeWrite
            );
        }
    }
    #[test]
    fn zero_is_not_missing_and_original_yogafan_is_not_validated() {
        let (cpu, gpu, mut a, b) = (
            value(43.5, TelemetryUnit::Celsius),
            value(40.0, TelemetryUnit::Celsius),
            fan("channel1"),
            fan("channel2"),
        );
        a.validation = FanValidation::Unvalidated;
        a.reported_speed.value = Some(0);
        let result =
            assess_trial_sample(&sample(&cpu, &gpu, [&a, &b]), TrialPhase::BeforeWrite, None);
        assert!(result.reasons.iter().any(|r| r.contains("unvalidated RPM")));
        assert_eq!(a.reported_speed.value, Some(0));
        a.validation = FanValidation::Validated;
        assert!(
            assess_trial_sample(&sample(&cpu, &gpu, [&a, &b]), TrialPhase::BeforeWrite, None)
                .reasons
                .iter()
                .any(|r| r.contains("experiment cutoff"))
        );
    }
    #[test]
    fn stale_lost_ac_missing_mapping_and_duplicate_channels_cancel_before_write() {
        let (cpu, gpu, a, b) = (
            value(43.5, TelemetryUnit::Celsius),
            value(40.0, TelemetryUnit::Celsius),
            fan("channel1"),
            fan("channel2"),
        );
        for scenario in 0..5 {
            let mut input = sample(&cpu, &gpu, [&a, &b]);
            match scenario {
                0 => input.ages_ms[1] = Some(3001),
                1 => input.ages_ms[0] = None,
                2 => input.ac_online = None,
                3 => input.independent_sensor_mapping_confirmed = false,
                _ => input.fans = [&a, &a],
            }
            assert_eq!(
                assess_trial_sample(&input, TrialPhase::BeforeWrite, None).disposition,
                TrialDisposition::CancelBeforeWrite
            );
        }
    }
    #[test]
    fn post_attempt_anomaly_is_neither_interrupt_nor_rollback() {
        let (cpu, gpu, a, b) = (
            value(43.5, TelemetryUnit::Celsius),
            value(40.0, TelemetryUnit::Celsius),
            fan("channel1"),
            fan("channel2"),
        );
        let baseline = sample(&cpu, &gpu, [&a, &b]);
        for temp in [53.5, 70.0, 100.0] {
            let hot = value(temp, TelemetryUnit::Celsius);
            let result = assess_trial_sample(
                &sample(&hot, &gpu, [&a, &b]),
                TrialPhase::AfterAttempt,
                Some(&baseline),
            );
            assert_eq!(
                result.disposition,
                TrialDisposition::AnomalyAfterAttemptNotRollback
            );
            assert!(!result.authorizes_write);
        }
        assert_eq!(
            assess_trial_sample(&baseline, TrialPhase::AfterAttempt, None).disposition,
            TrialDisposition::AnomalyAfterAttemptNotRollback
        );
    }
}
